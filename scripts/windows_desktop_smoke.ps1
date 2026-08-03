[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Binary
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Write-BoundedLogTail {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return
    }

    $stream = [System.IO.File]::Open(
        $Path,
        [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read,
        [System.IO.FileShare]::ReadWrite
    )
    try {
        $count = [int][Math]::Min([long]16384, $stream.Length)
        if ($count -eq 0) {
            return
        }

        [void]$stream.Seek(-$count, [System.IO.SeekOrigin]::End)
        $buffer = New-Object byte[] $count
        $read = 0
        while ($read -lt $count) {
            $received = $stream.Read($buffer, $read, $count - $read)
            if ($received -eq 0) {
                break
            }
            $read += $received
        }

        [Console]::Error.WriteLine([System.Text.Encoding]::UTF8.GetString($buffer, 0, $read))
    }
    finally {
        $stream.Dispose()
    }
}

$binaryPath = (Resolve-Path -LiteralPath $Binary).Path
if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
    throw "Windows desktop smoke binary is not a file: $Binary"
}

$smokeRoot = Join-Path ([System.IO.Path]::GetTempPath()) "codexrs-windows-smoke-$([guid]::NewGuid().ToString('N'))"
$codexHome = Join-Path $smokeRoot 'codex'
$dataDirectory = Join-Path $smokeRoot 'data\codexrs'
$stdoutLog = Join-Path $smokeRoot 'codexrs.stdout.log'
$stderrLog = Join-Path $smokeRoot 'codexrs.stderr.log'
$process = $null
$previousEnvironment = @{}

try {
    $null = New-Item -ItemType Directory -Path $codexHome -Force
    $null = New-Item -ItemType Directory -Path $dataDirectory -Force

    $falseCodexBinary = Join-Path $env:SystemRoot 'System32\where.exe'
    if (-not (Test-Path -LiteralPath $falseCodexBinary -PathType Leaf)) {
        throw "Windows desktop smoke cannot find harmless Codex stub: $falseCodexBinary"
    }

    $smokeEnvironment = @{
        CODEX_HOME = $codexHome
        CODEX_RS_DATA_DIR = $dataDirectory
        CODEX_RS_CODEX_BIN = $falseCodexBinary
    }
    foreach ($name in $smokeEnvironment.Keys) {
        $previousEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
        [Environment]::SetEnvironmentVariable($name, $smokeEnvironment[$name], 'Process')
    }

    $process = Start-Process `
        -FilePath $binaryPath `
        -PassThru `
        -RedirectStandardOutput $stdoutLog `
        -RedirectStandardError $stderrLog
    Start-Sleep -Seconds 5
    $process.Refresh()
    if ($process.HasExited) {
        throw "Windows desktop smoke exited during startup (status $($process.ExitCode))"
    }

    if (-not $process.CloseMainWindow()) {
        throw 'Windows desktop smoke did not create a closable main window'
    }
    if (-not $process.WaitForExit(10000)) {
        throw 'Windows desktop smoke did not exit within 10 seconds of the close request'
    }
    $process.Refresh()
    if ($process.ExitCode -ne 0) {
        throw "Windows desktop smoke exited with status $($process.ExitCode)"
    }
}
catch {
    Write-BoundedLogTail -Path $stdoutLog
    Write-BoundedLogTail -Path $stderrLog
    throw
}
finally {
    if ($null -ne $process) {
        try {
            $process.Refresh()
            if (-not $process.HasExited) {
                Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
                $null = $process.WaitForExit(5000)
            }
        }
        finally {
            $process.Dispose()
        }
    }

    foreach ($name in $previousEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $previousEnvironment[$name], 'Process')
    }
    if (Test-Path -LiteralPath $smokeRoot) {
        Remove-Item -LiteralPath $smokeRoot -Recurse -Force
    }
}
