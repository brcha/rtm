# PowerShell helpers of the Release workflow (.github/workflows/release.yml). Steps share no
# PowerShell state, so each step that needs them dot-sources this file:
#   . ./scripts/release-helpers.ps1
# Function definitions only; dot-sourcing runs them in the step's own scope.

# The order of releases: vA.B.P, and a beta vA.B.0-beta.N below its release; $null for any other
# tag.
function Get-VersionKey([string]$Tag) {
  if ($Tag -notmatch '^v?(\d+)\.(\d+)\.(\d+)(?:-beta\.(\d+))?$') { return $null }
  $fourth = if ($Matches[4]) { $Matches[4] } else { 65535 }
  [version]"$($Matches[1]).$($Matches[2]).$($Matches[3]).$fourth"
}

# Runs msiexec silently with a verbose log; fails on any exit code but success and reboot-required.
function Invoke-Msiexec([string[]] $arguments, [string] $log) {
  $p = Start-Process msiexec.exe -ArgumentList ($arguments + @('/qn', '/norestart', '/l*v', $log)) -Wait -PassThru
  if ($p.ExitCode -ne 0 -and $p.ExitCode -ne 3010) {
    Get-Content $log -Tail 60
    throw "msiexec $($arguments -join ' ') failed with exit code $($p.ExitCode)."
  }
}

# Installed products of that name, from the uninstall entries Windows Installer writes.
function Get-Installed {
  @(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -eq 'Rusty Todo.txt Manager' } |
    ForEach-Object { [pscustomobject]@{ Code = [string]$_.PSChildName; Version = [string]$_.DisplayVersion } })
}
