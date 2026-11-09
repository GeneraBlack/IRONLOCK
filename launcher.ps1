param(
    [ValidateSet("gui", "game", "engine", "stop")]
    [string]$Mode = "gui",
    [string]$ModPath = "rust-ra2"
)

$ErrorActionPreference = "Continue"
$WorkspaceRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $WorkspaceRoot

$ReleaseExe = Join-Path $WorkspaceRoot "engine\target\release\rts-client.exe"
$DebugExe   = Join-Path $WorkspaceRoot "engine\target\debug\rts-client.exe"
$CargoToml  = Join-Path $WorkspaceRoot "engine\Cargo.toml"
$StdoutLog  = Join-Path $WorkspaceRoot "client_stdout.log"
$StderrLog  = Join-Path $WorkspaceRoot "client_stderr.log"

function Get-RunningEngineProcesses {
    Get-Process -Name "rts-client", "rts-client-3d" -ErrorAction SilentlyContinue
}

function Stop-EngineProcess {
    $procs = Get-RunningEngineProcesses
    if ($procs) {
        foreach ($p in $procs) {
            try {
                Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
            } catch {}
        }
        Start-Sleep -Milliseconds 250
        return $true
    }
    return $false
}

function Start-EngineProcess {
    param(
        [bool]$EngineStudioMode,
        [string]$SelectedMod = "rust-ra2"
    )
    Stop-EngineProcess | Out-Null

    $exeToRun = $null
    if (Test-Path $ReleaseExe) {
        $exeToRun = $ReleaseExe
    } elseif (Test-Path $DebugExe) {
        $exeToRun = $DebugExe
    }

    $argList = @("--mod-path", $SelectedMod)
    if ($EngineStudioMode) {
        $argList += "--studio"
    }

    if ($exeToRun) {
        $proc = Start-Process -FilePath $exeToRun `
            -ArgumentList $argList `
            -WorkingDirectory $WorkspaceRoot `
            -RedirectStandardOutput $StdoutLog `
            -RedirectStandardError $StderrLog `
            -PassThru
        return $proc
    } else {
        $cargoArgs = @("run", "--manifest-path", $CargoToml, "--release", "-p", "rts-client", "--") + $argList
        $proc = Start-Process -FilePath "cargo" `
            -ArgumentList $cargoArgs `
            -WorkingDirectory $WorkspaceRoot `
            -RedirectStandardOutput $StdoutLog `
            -RedirectStandardError $StderrLog `
            -PassThru
        return $proc
    }
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
    $p = Start-EngineProcess -EngineStudioMode $false -SelectedMod $ModPath
    Write-Host "[GAME MODE] Gestartet (PID $($p.Id)) mit Mod '$ModPath'." -ForegroundColor Green
    exit 0
}
elseif ($Mode -eq "engine") {
    $p = Start-EngineProcess -EngineStudioMode $true -SelectedMod $ModPath
    Write-Host "[ENGINE MODE] Gestartet (PID $($p.Id)) mit Mod '$ModPath' (--studio)." -ForegroundColor Cyan
    exit 0
}

# GUI Mode (Windows Forms Control Center)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()

$script:ActiveModeName = "Kein Prozess aktiv"

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
$btnGame.Text = "GAME MODE`r`n(Spiel direkt starten)"
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
$btnStop.Text = "STOP ENGINE (Laufende Instanz beenden)"
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

function Update-UiStatus {
    $procs = Get-RunningEngineProcesses
    if ($procs) {
        $pids = ($procs | Select-Object -ExpandProperty Id) -join ", "
        $lblStatus.Text = "  LAEUFT: $($script:ActiveModeName) (PID: $pids)"
        $lblStatus.ForeColor = [System.Drawing.Color]::FromArgb(95, 235, 135)
        $btnStop.Enabled = $true
    } else {
        $script:ActiveModeName = "Engine gestoppt"
        $lblStatus.Text = "  BEREIT: Keine Engine-Instanz aktiv"
        $lblStatus.ForeColor = [System.Drawing.Color]::FromArgb(165, 175, 195)
        $btnStop.Enabled = $false
    }
}

$btnGame.Add_Click({
    $mod = $cmbMod.SelectedItem.ToString()
    $lblStatus.Text = "  STARTE GAME MODE ($mod)..."
    $lblStatus.ForeColor = [System.Drawing.Color]::FromArgb(245, 210, 90)
    $form.Refresh()
    $p = Start-EngineProcess -EngineStudioMode $false -SelectedMod $mod
    $script:ActiveModeName = "GAME MODE [$mod]"
    Update-UiStatus
})

$btnEngine.Add_Click({
    $mod = $cmbMod.SelectedItem.ToString()
    $lblStatus.Text = "  STARTE ENGINE MODE ($mod --studio)..."
    $lblStatus.ForeColor = [System.Drawing.Color]::FromArgb(100, 195, 255)
    $form.Refresh()
    $p = Start-EngineProcess -EngineStudioMode $true -SelectedMod $mod
    $script:ActiveModeName = "ENGINE MODE [$mod --studio]"
    Update-UiStatus
})

$btnStop.Add_Click({
    Stop-EngineProcess | Out-Null
    $script:ActiveModeName = "Engine gestoppt"
    Update-UiStatus
})

$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 1000
$timer.Add_Tick({ Update-UiStatus })
$timer.Start()

Update-UiStatus
[void]$form.ShowDialog()
$timer.Stop()
