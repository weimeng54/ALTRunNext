param(
    [Parameter(Mandatory=$true)][int]$TargetProcessId,
    [int]$Seconds=30,
    [Parameter(Mandatory=$true)][string]$OutputPath
)
$ErrorActionPreference='Stop'
$absolute=[IO.Path]::GetFullPath($OutputPath)
if(-not $absolute.StartsWith('D:\',[StringComparison]::OrdinalIgnoreCase)){throw 'Measurement output must stay on D:.'}
$parent=Split-Path -Parent $absolute
New-Item -ItemType Directory -Force $parent | Out-Null
$logical=[Environment]::ProcessorCount
$timer=[Diagnostics.Stopwatch]::StartNew()
$samples=[Collections.Generic.List[object]]::new()
$previous=Get-Process -Id $TargetProcessId
$lastCpu=$previous.TotalProcessorTime.TotalMilliseconds
$lastTime=$timer.Elapsed.TotalMilliseconds
while($timer.Elapsed.TotalSeconds -lt $Seconds){
    Start-Sleep -Milliseconds 200
    $p=Get-Process -Id $TargetProcessId
    $now=$timer.Elapsed.TotalMilliseconds
    $cpu=$p.TotalProcessorTime.TotalMilliseconds
    $samples.Add([pscustomobject]@{
        elapsed_ms=$now
        private_bytes=$p.PrivateMemorySize64
        working_set_bytes=$p.WorkingSet64
        cpu_percent_total=($cpu-$lastCpu)/($now-$lastTime)/$logical*100
        handles=$p.HandleCount
        threads=$p.Threads.Count
    })
    $lastTime=$now;$lastCpu=$cpu
}
$record=[pscustomobject]@{kind='resources-only';timestamp_utc=[DateTimeOffset]::UtcNow.ToString('o');pid=$TargetProcessId;duration_seconds=$timer.Elapsed.TotalSeconds;logical_processors=$logical;samples=$samples}
$record | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $absolute -Encoding utf8NoBOM
$private=$samples | Measure-Object -Property private_bytes -Minimum -Maximum -Average
$working=$samples | Measure-Object -Property working_set_bytes -Minimum -Maximum -Average
$cpu=$samples | Measure-Object -Property cpu_percent_total -Average -Maximum
[pscustomobject]@{samples=$samples.Count;private_mib_min=$private.Minimum/1MB;private_mib_max=$private.Maximum/1MB;working_set_mib_max=$working.Maximum/1MB;cpu_percent_total_mean=$cpu.Average;cpu_percent_total_max=$cpu.Maximum;output=$absolute} | ConvertTo-Json
