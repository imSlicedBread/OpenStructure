# Windows process-memory observation for the bounded synthetic history probe.
# Build the release example first. No project files or settings are modified.
param(
    [string]$ProbeExecutable = (Join-Path $PSScriptRoot '../work/history-measurement/release/examples/history_cost.exe')
)
$ErrorActionPreference = 'Stop'
$resolvedProbe = (Resolve-Path -LiteralPath $ProbeExecutable).Path
foreach ($probeMode in @('bounded', 'unbounded-reference')) {
    $launchOptions = @{
        FilePath = $resolvedProbe
        WindowStyle = 'Hidden'
        PassThru = $true
    }
    if ($probeMode -eq 'unbounded-reference') {
        $launchOptions.ArgumentList = @('--unbounded-reference')
    }
    $probeProcess = Start-Process @launchOptions
    $peakWorkingSet = 0L
    $peakPrivateBytes = 0L
    try {
        while (-not $probeProcess.HasExited) {
            $probeProcess.Refresh()
            if (-not $probeProcess.HasExited) {
                $peakWorkingSet = [Math]::Max($peakWorkingSet, $probeProcess.PeakWorkingSet64)
                $peakPrivateBytes = [Math]::Max($peakPrivateBytes, $probeProcess.PrivateMemorySize64)
            }
            [void]$probeProcess.WaitForExit(5)
        }
        $probeProcess.WaitForExit()
        if ($probeProcess.ExitCode -ne 0) {
            throw "History probe failed ($probeMode), exit $($probeProcess.ExitCode)"
        }
        [pscustomobject]@{
            Mode = $probeMode
            ObservedPeakWorkingSetBytes = $peakWorkingSet
            SampledPeakPrivateBytes = $peakPrivateBytes
            PollIntervalMilliseconds = 5
        }
    }
    finally {
        $probeProcess.Dispose()
    }
}
