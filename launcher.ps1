param(
    [ValidateSet("gui", "game", "engine", "stop")]
    [string]$Mode = "gui",
    [string]$ModPath = "rust-ra2",
    [switch]$ReplaceExisting
)

$ErrorActionPreference = "Continue"
$WorkspaceRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $WorkspaceRoot

$ReleaseExe = Join-Path $WorkspaceRoot "engine\target\release\rts-client.exe"
$CargoToml  = Join-Path $WorkspaceRoot "engine\Cargo.toml"

function Get-RunningEngineProcesses {
    Get-Process -Name "rts-client", "rts-client-3d" -ErrorAction SilentlyContinue
}

function Resolve-LogPaths {
    param([int]$InstanceIndex = 1)
    if ($InstanceIndex -le 1) {
        $outPath = Join-Path $WorkspaceRoot "client_stdout.log"
        $errPath = Join-Path $WorkspaceRoot "client_stderr.log"
    } else {
        $stamp = "$InstanceIndex-$(Get-Date -Format 'HHmmss')"
        $outPath = Join-Path $WorkspaceRoot "client_stdout_$stamp.log"
        $errPath = Join-Path $WorkspaceRoot "client_stderr_$stamp.log"
    }
    return @{ Stdout = $outPath; Stderr = $errPath }
}

function Stop-EngineProcess {
    $procs = @(Get-RunningEngineProcesses)
    if ($procs.Count -gt 0) {
        foreach ($p in $procs) {
            try {
                if ($p.MainWindowHandle -ne [IntPtr]::Zero) {
                    [void]$p.CloseMainWindow()
                }
            } catch {}
        }

        $deadline = (Get-Date).AddMilliseconds(800)
        while ((Get-Date) -lt $deadline) {
            if (@(Get-RunningEngineProcesses).Count -eq 0) {
                return $true
            }
            Start-Sleep -Milliseconds 100
        }

        foreach ($p in @(Get-RunningEngineProcesses)) {
            try {
                Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
            } catch {}
        }
        Start-Sleep -Milliseconds 150
        return $true
    }
    return $false
}

function Invoke-ReleaseBuildSync {
    Write-Host "[BUILD] COMPILING rts-client (--release)..." -ForegroundColor Yellow
    & cargo build --manifest-path $CargoToml --release -p rts-client
    $code = $LASTEXITCODE
    if ($code -ne 0) {
        Write-Error "[ERROR] Cargo release build failed with exit code $code."
        return $code
    }
    if (-not (Test-Path $ReleaseExe)) {
        Write-Error "[ERROR] Expected release binary not found at '$ReleaseExe'."
        return 1
    }
    return 0
}

function Start-EngineProcess {
    param(
        [bool]$EngineStudioMode,
        [string]$SelectedMod = "rust-ra2",
        [bool]$StopRunning = $false
    )
    if ($StopRunning) {
        Stop-EngineProcess | Out-Null
    }

    if (-not (Test-Path $ReleaseExe)) {
        $buildCode = Invoke-ReleaseBuildSync
        if ($buildCode -ne 0) {
            throw "Cargo release build failed (ExitCode: $buildCode)"
        }
    }

    $runningCount = @(Get-RunningEngineProcesses).Count
    $logs = Resolve-LogPaths -InstanceIndex ($runningCount + 1)

    $argList = @("--mod-path", $SelectedMod)
    if ($EngineStudioMode) {
        $argList += "--studio"
    }

    $proc = Start-Process -FilePath $ReleaseExe `
        -ArgumentList $argList `
        -WorkingDirectory $WorkspaceRoot `
        -RedirectStandardOutput $logs.Stdout `
        -RedirectStandardError $logs.Stderr `
        -PassThru

    Start-Sleep -Milliseconds 200
    if ($proc.HasExited -and $proc.ExitCode -ne 0) {
        throw "rts-client exited immediately with code $($proc.ExitCode). Check $($logs.Stderr)."
    }

    return $proc
}

# Direct CLI mode support: .\launcher.ps1 game | engine | stop
if ($Mode -eq "stop") {
    $stopped = Stop-EngineProcess
    if ($stopped) {
        Write-Host "[STOP] Rust-RTS Engine wurde beendet." -ForegroundColor Yellow
    } else {
        Write-Host "[INFO] Keine laufende Rust-RTS Engine gefunden." -ForegroundColor Gray
    }
    exit 0
}
elseif ($Mode -eq "game") {
    try {
        $p = Start-EngineProcess -EngineStudioMode $false -SelectedMod $ModPath -StopRunning ([bool]$ReplaceExisting)
        Write-Host "[GAME MODE] Gestartet (PID $($p.Id)) mit Mod '$ModPath'." -ForegroundColor Green
        exit 0
    } catch {
        Write-Error $_
        exit 1
    }
}
elseif ($Mode -eq "engine") {
    try {
        $p = Start-EngineProcess -EngineStudioMode $true -SelectedMod $ModPath -StopRunning ([bool]$ReplaceExisting)
        Write-Host "[ENGINE MODE] Gestartet (PID $($p.Id)) mit Mod '$ModPath' (--studio)." -ForegroundColor Cyan
        exit 0
    } catch {
        Write-Error $_
        exit 1
    }
}

# GUI Mode (Windows Forms Control Center)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()

$script:ActiveModeName = "Kein Prozess aktiv"
$script:BuildProcess = $null
$script:PendingLaunchStudio = $false
$script:PendingLaunchMod = "rust-ra2"
$script:LastBuildError = $null

$form = New-Object System.Windows.Forms.Form
$form.Text = "IRONLOCK // RUST-RTS - Start & Stop Control Panel"
$form.Size = New-Object System.Drawing.Size(500, 340)
$form.StartPosition = "CenterScreen"
$form.FormBorderStyle = "FixedDialog"
$form.MaximizeBox = $false
$form.BackColor = [System.Drawing.Color]::FromArgb(20, 25, 34)
$form.ForeColor = [System.Drawing.Color]::FromArgb(235, 240, 250)

$fontTitle  = New-Object System.Drawing.Font("Segoe UI", 13, [System.Drawing.FontStyle]::Bold)
$fontSub    = New-Object System.Drawing.Font("Segoe UI", 9, [System.Drawing.FontStyle]::Regular)
$fontBtn    = New-Object System.Drawing.Font("Segoe UI", 11, [System.Drawing.FontStyle]::Bold)
$fontStatus = New-Object System.Drawing.Font("Consolas", 9.5, [System.Drawing.FontStyle]::Bold)

$lblHeader = New-Object System.Windows.Forms.Label
$lblHeader.Text = "IRONLOCK // RUST-RTS LAUNCHER"
$lblHeader.Font = $fontTitle
$lblHeader.ForeColor = [System.Drawing.Color]::FromArgb(245, 200, 75)
$lblHeader.Location = New-Object System.Drawing.Point(22, 16)
$lblHeader.Size = New-Object System.Drawing.Size(440, 28)
$form.Controls.Add($lblHeader)

$lblMod = New-Object System.Windows.Forms.Label
$lblMod.Text = "Aktives Mod-Paket:"
$lblMod.Font = $fontSub
$lblMod.ForeColor = [System.Drawing.Color]::FromArgb(180, 192, 210)
$lblMod.Location = New-Object System.Drawing.Point(24, 52)
$lblMod.Size = New-Object System.Drawing.Size(120, 22)
$form.Controls.Add($lblMod)

$cmbMod = New-Object System.Windows.Forms.ComboBox
$cmbMod.DropDownStyle = "DropDownList"
$cmbMod.Font = $fontSub
$cmbMod.BackColor = [System.Drawing.Color]::FromArgb(32, 40, 54)
$cmbMod.ForeColor = [System.Drawing.Color]::White
$cmbMod.Location = New-Object System.Drawing.Point(145, 49)
$cmbMod.Size = New-Object System.Drawing.Size(315, 26)
[void]$cmbMod.Items.Add("rust-ra2")
[void]$cmbMod.Items.Add("mods/openra2")
[void]$cmbMod.Items.Add("mods/westwood-classic")
[void]$cmbMod.Items.Add("mods/openra-classic")
$cmbMod.SelectedIndex = 0
$form.Controls.Add($cmbMod)

# Button 1: GAME MODE
$btnGame = New-Object System.Windows.Forms.Button
$btnGame.Text = "GAME MODE`r`n(Spiel / Peer starten)"
$btnGame.Font = $fontBtn
$btnGame.FlatStyle = "Flat"
$btnGame.FlatAppearance.BorderSize = 1
$btnGame.FlatAppearance.BorderColor = [System.Drawing.Color]::FromArgb(70, 195, 115)
$btnGame.BackColor = [System.Drawing.Color]::FromArgb(28, 115, 65)
$btnGame.ForeColor = [System.Drawing.Color]::White
$btnGame.Location = New-Object System.Drawing.Point(24, 92)
$btnGame.Size = New-Object System.Drawing.Size(210, 86)
$btnGame.Cursor = [System.Windows.Forms.Cursors]::Hand
$form.Controls.Add($btnGame)

# Button 2: ENGINE MODE
$btnEngine = New-Object System.Windows.Forms.Button
$btnEngine.Text = "ENGINE MODE`r`n(Studio, Editor & Tools)"
$btnEngine.Font = $fontBtn
$btnEngine.FlatStyle = "Flat"
$btnEngine.FlatAppearance.BorderSize = 1
$btnEngine.FlatAppearance.BorderColor = [System.Drawing.Color]::FromArgb(85, 165, 245)
$btnEngine.BackColor = [System.Drawing.Color]::FromArgb(32, 92, 168)
$btnEngine.ForeColor = [System.Drawing.Color]::White
$btnEngine.Location = New-Object System.Drawing.Point(250, 92)
$btnEngine.Size = New-Object System.Drawing.Size(210, 86)
$btnEngine.Cursor = [System.Windows.Forms.Cursors]::Hand
$form.Controls.Add($btnEngine)

# Button 3: STOP ENGINE
$btnStop = New-Object System.Windows.Forms.Button
$btnStop.Text = "STOP ENGINE (Laufende Instanzen beenden)"
$btnStop.Font = $fontBtn
$btnStop.FlatStyle = "Flat"
$btnStop.FlatAppearance.BorderSize = 1
$btnStop.FlatAppearance.BorderColor = [System.Drawing.Color]::FromArgb(225, 85, 85)
$btnStop.BackColor = [System.Drawing.Color]::FromArgb(145, 38, 42)
$btnStop.ForeColor = [System.Drawing.Color]::White
$btnStop.Location = New-Object System.Drawing.Point(24, 192)
$btnStop.Size = New-Object System.Drawing.Size(436, 46)
$btnStop.Cursor = [System.Windows.Forms.Cursors]::Hand
$form.Controls.Add($btnStop)

# Status Bar Panel
$lblStatus = New-Object System.Windows.Forms.Label
$lblStatus.Text = "STATUS: Prüfe..."
$lblStatus.Font = $fontStatus
$lblStatus.BackColor = [System.Drawing.Color]::FromArgb(14, 18, 25)
$lblStatus.ForeColor = [System.Drawing.Color]::FromArgb(160, 175, 195)
$lblStatus.BorderStyle = "FixedSingle"
$lblStatus.TextAlign = "MiddleLeft"
$lblStatus.Location = New-Object System.Drawing.Point(24, 252)
$lblStatus.Size = New-Object System.Drawing.Size(436, 36)
$form.Controls.Add($lblStatus)

function Start-GuiLaunchOrCompile {
    param([bool]$StudioMode, [string]$SelectedMod)
    $script:LastBuildError = $null
    if ($script:BuildProcess -and -not $script:BuildProcess.HasExited) {
        return
    }

    if (-not (Test-Path $ReleaseExe)) {
        $script:PendingLaunchStudio = $StudioMode
        $script:PendingLaunchMod = $SelectedMod
        $buildOut = Join-Path $WorkspaceRoot "cargo_build_stdout.log"
        $buildErr = Join-Path $WorkspaceRoot "cargo_build_stderr.log"
        $script:BuildProcess = Start-Process -FilePath "cargo" `
            -ArgumentList @("build", "--manifest-path", $CargoToml, "--release", "-p", "rts-client") `
            -WorkingDirectory $WorkspaceRoot `
            -RedirectStandardOutput $buildOut `
            -RedirectStandardError $buildErr `
            -NoNewWindow `
            -PassThru
        Update-UiStatus
        return
    }

    try {
        [void](Start-EngineProcess -EngineStudioMode $StudioMode -SelectedMod $SelectedMod -StopRunning $false)
        $script:ActiveModeName = if ($StudioMode) { "ENGINE MODE [$SelectedMod --studio]" } else { "GAME MODE [$SelectedMod]" }
    } catch {
        $script:LastBuildError = $_.Exception.Message
    }
    Update-UiStatus
}

function Update-UiStatus {
    if ($script:BuildProcess) {
        if (-not $script:BuildProcess.HasExited) {
            $lblStatus.Text = "  COMPILING... Cargo Release-Build (PID: $($script:BuildProcess.Id))"
            $lblStatus.ForeColor = [System.Drawing.Color]::FromArgb(245, 210, 90)
            $btnGame.Enabled = $false
            $btnEngine.Enabled = $false
            $btnStop.Enabled = $true
            return
        } else {
            $exitCode = $script:BuildProcess.ExitCode
            $script:BuildProcess = $null
            $btnGame.Enabled = $true
            $btnEngine.Enabled = $true
            if ($exitCode -eq 0 -and (Test-Path $ReleaseExe)) {
                Start-GuiLaunchOrCompile -StudioMode $script:PendingLaunchStudio -SelectedMod $script:PendingLaunchMod
                return
            } else {
                $script:LastBuildError = "Cargo Build fehlgeschlagen (ExitCode: $exitCode)"
            }
        }
    }

    $procs = @(Get-RunningEngineProcesses)
    if ($procs.Count -gt 0) {
        $pids = ($procs | Select-Object -ExpandProperty Id) -join ", "
        $lblStatus.Text = "  LAEUFT ($($procs.Count)x): $($script:ActiveModeName) (PID: $pids)"
        $lblStatus.ForeColor = [System.Drawing.Color]::FromArgb(95, 235, 135)
        $btnStop.Enabled = $true
    } elseif ($script:LastBuildError) {
        $lblStatus.Text = "  FEHLER: $($script:LastBuildError)"
        $lblStatus.ForeColor = [System.Drawing.Color]::FromArgb(245, 105, 105)
        $btnStop.Enabled = $false
    } else {
        $script:ActiveModeName = "Engine gestoppt"
        $lblStatus.Text = "  BEREIT: Keine Engine-Instanz aktiv"
        $lblStatus.ForeColor = [System.Drawing.Color]::FromArgb(165, 175, 195)
        $btnStop.Enabled = $false
    }
}

$btnGame.Add_Click({
    $mod = $cmbMod.SelectedItem.ToString()
    Start-GuiLaunchOrCompile -StudioMode $false -SelectedMod $mod
})

$btnEngine.Add_Click({
    $mod = $cmbMod.SelectedItem.ToString()
    Start-GuiLaunchOrCompile -StudioMode $true -SelectedMod $mod
})

$btnStop.Add_Click({
    if ($script:BuildProcess -and -not $script:BuildProcess.HasExited) {
        try {
            Stop-Process -Id $script:BuildProcess.Id -Force -ErrorAction SilentlyContinue
        } catch {}
        $script:BuildProcess = $null
    }
    Stop-EngineProcess | Out-Null
    $script:LastBuildError = $null
    $script:ActiveModeName = "Engine gestoppt"
    $btnGame.Enabled = $true
    $btnEngine.Enabled = $true
    Update-UiStatus
})

$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 750
$timer.Add_Tick({ Update-UiStatus })
$timer.Start()

Update-UiStatus
[void]$form.ShowDialog()
$timer.Stop()
