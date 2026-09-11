use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

#[cfg(windows)]
use std::process::Command;

use sysinfo::{ProcessRefreshKind, RefreshKind, System};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessSample {
    pub pid: u32,
    pub parent_pid: u32,
    pub rss_bytes: u64,
    pub private_bytes: Option<u64>,
    pub private_working_set_bytes: Option<u64>,
}

#[derive(Clone, Debug, Default)]
struct GpuMemorySample {
    dedicated_bytes: Option<u64>,
    shared_bytes: Option<u64>,
}

#[derive(Clone, Debug, Default)]
struct GpuCollection {
    memory: BTreeMap<u32, GpuMemorySample>,
    engine_utilization: BTreeMap<u32, BTreeMap<String, f64>>,
    error: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct GpuSnapshot {
    dedicated_bytes: Option<u64>,
    shared_bytes: Option<u64>,
    memory_coverage: usize,
    engine_coverage: usize,
    utilization_by_engine: BTreeMap<String, f64>,
    error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MetricCollection {
    values: BTreeMap<u32, u64>,
    error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ResourceSnapshot {
    pub root_pid: u32,
    pub processes: Vec<ProcessSample>,
    pub rss_bytes: u64,
    pub private_bytes: Option<u64>,
    pub private_working_set_bytes: Option<u64>,
    gpu: GpuSnapshot,
    private_bytes_error: Option<String>,
    private_working_set_error: Option<String>,
}

impl ResourceSnapshot {
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        let process_count = self.processes.len();
        let private_bytes_coverage = self
            .processes
            .iter()
            .filter(|sample| sample.private_bytes.is_some())
            .count();
        let private_working_set_coverage = self
            .processes
            .iter()
            .filter(|sample| sample.private_working_set_bytes.is_some())
            .count();
        let gpu_memory_available =
            self.gpu.dedicated_bytes.is_some() || self.gpu.shared_bytes.is_some();
        let gpu_engine_available = !self.gpu.utilization_by_engine.is_empty();
        serde_json::json!({
            "root_pid": self.root_pid,
            "platform": std::env::consts::OS,
            "process_count": process_count,
            "rss_bytes": self.rss_bytes,
            "private_bytes": self.private_bytes,
            "private_working_set_bytes": self.private_working_set_bytes,
            "gpu": {
                "memory": {
                    "counter_available": self.gpu.error.is_none(),
                    "available": gpu_memory_available,
                    "complete_coverage": self.gpu.memory_coverage == process_count
                        && gpu_memory_available,
                    "coverage": self.gpu.memory_coverage,
                    "dedicated_bytes": self.gpu.dedicated_bytes,
                    "shared_bytes": self.gpu.shared_bytes,
                    "definition": "Windows GPU process dedicated/shared memory; unavailable when the process has no GPU allocation",
                    "error": self.gpu.error
                },
                "engine_utilization": {
                    "counter_available": self.gpu.error.is_none(),
                    "available": gpu_engine_available,
                    "coverage": self.gpu.engine_coverage,
                    "by_engine_type": self.gpu.utilization_by_engine,
                    "definition": "maximum observed utilization per engine type; values are not summed across engine types",
                    "error": self.gpu.error
                }
            },
            "metrics": {
                "rss_bytes": {
                    "available": true,
                    "coverage": process_count,
                    "definition": "resident set size; shared pages may be counted in more than one process"
                },
                "private_bytes": {
                    "available": private_bytes_coverage > 0,
                    "complete_coverage": self.private_bytes.is_some(),
                    "coverage": private_bytes_coverage,
                    "definition": "private committed virtual memory; this is not private working set",
                    "error": self.private_bytes_error
                },
                "private_working_set_bytes": {
                    "available": private_working_set_coverage > 0,
                    "complete_coverage": self.private_working_set_bytes.is_some(),
                    "coverage": private_working_set_coverage,
                    "definition": "physical working-set pages private to each process",
                    "error": self.private_working_set_error
                }
            },
            "processes": self.processes.iter().map(|sample| serde_json::json!({
                "pid": sample.pid,
                "parent_pid": sample.parent_pid,
                "rss_bytes": sample.rss_bytes,
                "private_bytes": sample.private_bytes,
                "private_working_set_bytes": sample.private_working_set_bytes,
            })).collect::<Vec<_>>(),
        })
    }
}

pub fn resource_report(args: &[String]) -> ExitCode {
    let pid = option_value(args, "--pid")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or_else(std::process::id);
    let report = match collect_resource_snapshot(pid) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("resource-report failed: {error}");
            return ExitCode::from(1);
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report.to_json()).expect("resource report serializes")
    );
    ExitCode::SUCCESS
}

pub fn collect_resource_snapshot(root_pid: u32) -> Result<ResourceSnapshot, String> {
    let mut samples = collect_process_samples();
    let private_bytes = collect_private_bytes();
    let private_working_set = collect_private_working_set();
    let gpu = collect_gpu_metrics();
    apply_metric(&mut samples, &private_bytes, |sample, value| {
        sample.private_bytes = value;
    });
    apply_metric(&mut samples, &private_working_set, |sample, value| {
        sample.private_working_set_bytes = value;
    });

    let processes = process_tree(&samples, root_pid);
    if processes.is_empty() {
        return Err(format!("process {root_pid} was not found"));
    }
    let gpu = gpu.for_process_tree(&processes);
    Ok(ResourceSnapshot {
        root_pid,
        rss_bytes: processes.iter().map(|sample| sample.rss_bytes).sum(),
        private_bytes: sum_optional(&processes, |sample| sample.private_bytes),
        private_working_set_bytes: sum_optional(&processes, |sample| {
            sample.private_working_set_bytes
        }),
        gpu,
        processes,
        private_bytes_error: private_bytes.error,
        private_working_set_error: private_working_set.error,
    })
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn collect_process_samples() -> Vec<ProcessSample> {
    let system = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );
    system
        .processes()
        .iter()
        .map(|(pid, process)| ProcessSample {
            pid: pid.as_u32(),
            parent_pid: process.parent().map_or(0, sysinfo::Pid::as_u32),
            rss_bytes: process.memory(),
            private_bytes: None,
            private_working_set_bytes: None,
        })
        .collect()
}

fn apply_metric(
    samples: &mut [ProcessSample],
    metric: &MetricCollection,
    mut apply: impl FnMut(&mut ProcessSample, Option<u64>),
) {
    for sample in samples {
        apply(sample, metric.values.get(&sample.pid).copied());
    }
}

fn sum_optional(
    processes: &[ProcessSample],
    value: impl Fn(&ProcessSample) -> Option<u64>,
) -> Option<u64> {
    processes.iter().map(value).try_fold(0_u64, |total, value| {
        value.map(|bytes| total.saturating_add(bytes))
    })
}

#[cfg(windows)]
fn collect_private_bytes() -> MetricCollection {
    collect_windows_metric(
        r"Get-Process -ErrorAction SilentlyContinue | ForEach-Object {
  try {
    [PSCustomObject]@{
      pid = [int]$_.Id
      value = [int64]$_.PrivateMemorySize64
    }
  } catch {}
} | ConvertTo-Json -Compress",
    )
}

#[cfg(not(windows))]
fn collect_private_bytes() -> MetricCollection {
    unavailable_metric("private bytes are not collected on this platform")
}

#[cfg(windows)]
fn collect_private_working_set() -> MetricCollection {
    collect_windows_metric(
        r"$counter = Get-Counter '\Process(*)\ID Process','\Process(*)\Working Set - Private' -ErrorAction SilentlyContinue
if ($null -eq $counter) {
  throw 'Windows process performance counters are unavailable'
}
$samples = $counter.CounterSamples | Where-Object { $_.Status -eq 0 }
$ids = @{}
$workingSets = @{}
foreach ($sample in $samples) {
  if ($sample.Path -notmatch '\\process\(([^)]+)\)\\') {
    continue
  }
  $instanceKey = $Matches[1]
  if ($sample.Path.EndsWith('\id process', [System.StringComparison]::OrdinalIgnoreCase)) {
    $ids[$instanceKey] = [int]$sample.CookedValue
  } elseif ($sample.Path.EndsWith('\working set - private', [System.StringComparison]::OrdinalIgnoreCase)) {
    $workingSets[$instanceKey] = [int64]$sample.CookedValue
  }
}
$ids.GetEnumerator() | ForEach-Object {
  if ($_.Value -gt 0 -and $workingSets.ContainsKey($_.Key)) {
    [PSCustomObject]@{ pid = [int]$_.Value; value = [int64]$workingSets[$_.Key] }
  }
} | ConvertTo-Json -Compress",
    )
}

#[cfg(not(windows))]
fn collect_private_working_set() -> MetricCollection {
    unavailable_metric("private working set is not collected on this platform")
}

fn unavailable_metric(error: &str) -> MetricCollection {
    MetricCollection {
        values: BTreeMap::new(),
        error: Some(error.to_owned()),
    }
}

#[cfg(windows)]
const GPU_COUNTER_SCRIPT: &str = r"$counter = Get-Counter '\GPU Process Memory(*)\Dedicated Usage','\GPU Process Memory(*)\Shared Usage','\GPU Engine(*)\Utilization Percentage' -ErrorAction SilentlyContinue
if ($null -eq $counter) {
  throw 'Windows GPU counters are unavailable'
}
$rows = foreach ($sample in $counter.CounterSamples | Where-Object { $_.Status -eq 0 }) {
  if ($sample.Path -match '\\gpu process memory\(pid_(\d+)_.*\)\\(dedicated usage|shared usage)$') {
    [PSCustomObject]@{ kind = $Matches[2]; pid = [int]$Matches[1]; value = [int64]$sample.CookedValue }
  } elseif ($sample.Path -match '\\gpu engine\(pid_(\d+)_.*_engtype_(.*)\)\\utilization percentage$') {
    $engine = $Matches[2].Trim()
    if ([string]::IsNullOrWhiteSpace($engine)) { $engine = 'unknown' }
    [PSCustomObject]@{ kind = 'engine:' + $engine; pid = [int]$Matches[1]; value = [double]$sample.CookedValue }
  }
}
if ($null -eq $rows) { '[]' } else { @($rows) | ConvertTo-Json -Compress }";

#[cfg(windows)]
fn collect_gpu_metrics() -> GpuCollection {
    let output = match Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            GPU_COUNTER_SCRIPT,
        ])
        .output()
    {
        Ok(output) => output,
        Err(error) => return unavailable_gpu(&error.to_string()),
    };
    if !output.status.success() {
        return unavailable_gpu(String::from_utf8_lossy(&output.stderr).trim());
    }
    match parse_gpu_samples(&output.stdout) {
        Ok(samples) => samples,
        Err(error) => unavailable_gpu(&error),
    }
}

#[cfg(not(windows))]
fn collect_gpu_metrics() -> GpuCollection {
    unavailable_gpu("GPU counters are not collected on this platform")
}

fn unavailable_gpu(error: &str) -> GpuCollection {
    GpuCollection {
        error: Some(error.to_owned()),
        ..GpuCollection::default()
    }
}

impl GpuCollection {
    fn for_process_tree(&self, processes: &[ProcessSample]) -> GpuSnapshot {
        let process_ids = processes
            .iter()
            .map(|sample| sample.pid)
            .collect::<BTreeSet<_>>();
        let mut memory_coverage = 0;
        let mut dedicated_bytes = Some(0_u64);
        let mut shared_bytes = Some(0_u64);
        for (pid, sample) in &self.memory {
            if !process_ids.contains(pid) {
                continue;
            }
            memory_coverage += 1;
            dedicated_bytes = add_optional(dedicated_bytes, sample.dedicated_bytes);
            shared_bytes = add_optional(shared_bytes, sample.shared_bytes);
        }
        if memory_coverage == 0 {
            dedicated_bytes = None;
            shared_bytes = None;
        }

        let mut engine_coverage = 0;
        let mut utilization_by_engine = BTreeMap::new();
        for (pid, engines) in &self.engine_utilization {
            if !process_ids.contains(pid) {
                continue;
            }
            engine_coverage += 1;
            for (engine, value) in engines {
                let entry = utilization_by_engine
                    .entry(engine.clone())
                    .or_insert(0.0_f64);
                *entry = entry.max(*value);
            }
        }

        GpuSnapshot {
            dedicated_bytes,
            shared_bytes,
            memory_coverage,
            engine_coverage,
            utilization_by_engine,
            error: self.error.clone(),
        }
    }
}

fn add_optional(total: Option<u64>, value: Option<u64>) -> Option<u64> {
    Some(total?.saturating_add(value?))
}

#[cfg(windows)]
fn parse_gpu_samples(bytes: &[u8]) -> Result<GpuCollection, String> {
    let text = String::from_utf8_lossy(bytes);
    if text.trim().is_empty() {
        return Ok(GpuCollection::default());
    }
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let values = match value {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Null => Vec::new(),
        value => vec![value],
    };
    let mut collection = GpuCollection::default();
    for value in values {
        let pid = value
            .get("pid")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| "GPU process metric pid is invalid".to_owned())?;
        let kind = value
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "GPU metric kind is invalid".to_owned())?;
        if let Some(engine) = kind.strip_prefix("engine:") {
            let metric = value
                .get("value")
                .and_then(serde_json::Value::as_f64)
                .filter(|metric| metric.is_finite() && *metric >= 0.0)
                .ok_or_else(|| "GPU engine utilization is invalid".to_owned())?;
            collection
                .engine_utilization
                .entry(pid)
                .or_default()
                .entry(engine.to_owned())
                .and_modify(|current| *current = current.max(metric))
                .or_insert(metric);
            continue;
        }

        let metric = value
            .get("value")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "GPU memory value is invalid".to_owned())?;
        let sample = collection.memory.entry(pid).or_default();
        match kind {
            "dedicated usage" => {
                sample.dedicated_bytes = Some(
                    sample
                        .dedicated_bytes
                        .unwrap_or_default()
                        .saturating_add(metric),
                );
            }
            "shared usage" => {
                sample.shared_bytes = Some(
                    sample
                        .shared_bytes
                        .unwrap_or_default()
                        .saturating_add(metric),
                );
            }
            _ => return Err(format!("unknown GPU metric kind: {kind}")),
        }
    }
    Ok(collection)
}

#[cfg(windows)]
fn collect_windows_metric(script: &str) -> MetricCollection {
    let output = match Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
    {
        Ok(output) => output,
        Err(error) => return unavailable_metric(&error.to_string()),
    };
    if !output.status.success() {
        return unavailable_metric(String::from_utf8_lossy(&output.stderr).trim());
    }
    match parse_metric_samples(&output.stdout) {
        Ok(values) => MetricCollection {
            values,
            error: None,
        },
        Err(error) => unavailable_metric(&error),
    }
}

#[cfg(windows)]
fn parse_metric_samples(bytes: &[u8]) -> Result<BTreeMap<u32, u64>, String> {
    let text = String::from_utf8_lossy(bytes);
    if text.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let values = match value {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Null => Vec::new(),
        value => vec![value],
    };
    values
        .into_iter()
        .map(|value| {
            let pid = value
                .get("pid")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| "process metric pid is invalid".to_owned())?;
            let bytes = value
                .get("value")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| "process metric value is invalid".to_owned())?;
            Ok((pid, bytes))
        })
        .collect()
}

fn process_tree(samples: &[ProcessSample], root_pid: u32) -> Vec<ProcessSample> {
    let mut children = BTreeMap::<u32, Vec<u32>>::new();
    for sample in samples {
        children
            .entry(sample.parent_pid)
            .or_default()
            .push(sample.pid);
    }
    let mut pending = vec![root_pid];
    let mut included = BTreeSet::new();
    while let Some(pid) = pending.pop() {
        if !included.insert(pid) {
            continue;
        }
        if let Some(child_pids) = children.get(&pid) {
            pending.extend(child_pids.iter().copied());
        }
    }
    samples
        .iter()
        .filter(|sample| included.contains(&sample.pid))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ProcessSample, process_tree, sum_optional};

    fn sample(
        pid: u32,
        parent_pid: u32,
        rss_bytes: u64,
        private_bytes: Option<u64>,
    ) -> ProcessSample {
        ProcessSample {
            pid,
            parent_pid,
            rss_bytes,
            private_bytes,
            private_working_set_bytes: private_bytes,
        }
    }

    #[test]
    fn process_tree_keeps_root_descendants_and_excludes_unrelated_processes() {
        let samples = vec![
            sample(10, 1, 100, Some(90)),
            sample(11, 10, 200, Some(190)),
            sample(12, 11, 300, Some(290)),
            sample(20, 1, 400, Some(390)),
        ];
        let tree = process_tree(&samples, 10);
        assert_eq!(
            tree.iter().map(|sample| sample.pid).collect::<Vec<_>>(),
            vec![10, 11, 12]
        );
    }

    #[test]
    fn optional_metric_sum_requires_complete_tree_coverage() {
        let complete = vec![sample(10, 1, 100, Some(90)), sample(11, 10, 200, Some(190))];
        assert_eq!(
            sum_optional(&complete, |sample| sample.private_bytes),
            Some(280)
        );

        let partial = vec![sample(10, 1, 100, Some(90)), sample(11, 10, 200, None)];
        assert_eq!(sum_optional(&partial, |sample| sample.private_bytes), None);
    }

    #[cfg(windows)]
    #[test]
    fn windows_metric_samples_parse_single_json_object() {
        let samples = super::parse_metric_samples(br#"{"pid":10,"value":180}"#)
            .expect("valid process metric");
        assert_eq!(samples.get(&10), Some(&180));
    }

    #[cfg(windows)]
    #[test]
    fn gpu_samples_filter_the_tree_and_keep_engine_types_separate() {
        let samples = super::parse_gpu_samples(
            br#"[
                {"kind":"dedicated usage","pid":10,"value":100},
                {"kind":"shared usage","pid":10,"value":200},
                {"kind":"engine:3d","pid":10,"value":1.5},
                {"kind":"engine:copy","pid":10,"value":2.5},
                {"kind":"shared usage","pid":20,"value":999}
            ]"#,
        )
        .expect("valid GPU process metrics");
        let snapshot = samples.for_process_tree(&[sample(10, 1, 100, Some(90))]);

        assert_eq!(snapshot.dedicated_bytes, Some(100));
        assert_eq!(snapshot.shared_bytes, Some(200));
        assert_eq!(snapshot.memory_coverage, 1);
        assert_eq!(snapshot.engine_coverage, 1);
        assert!((snapshot.utilization_by_engine["3d"] - 1.5).abs() < f64::EPSILON);
        assert!((snapshot.utilization_by_engine["copy"] - 2.5).abs() < f64::EPSILON);
    }
}
