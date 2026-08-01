# Fresh README screenshots of the current UI, scripted so they can be regenerated.
#
# Headless Edge rather than a screenshot tool: it takes an exact viewport, waits
# for the force layout to settle via --virtual-time-budget, and writes a file we
# can commit. The old screenshots were from the "Banyan" era — wrong name, wrong
# colours, and a three-pane layout deleted in Phase 16.
#
# IMPORTANT: point this at a server serving the DEMO vault from demo-vault.sh, not
# at whatever vault you happen to have registered. Running it against a real vault
# bakes that vault's note titles and folder names into images committed to a public
# repository — which is how private material leaks without anyone deciding to leak
# it. There is no default port for that reason: pass one deliberately.
#
#   bash docs/demo-vault.sh /tmp/demo
#   SAMONG_CONFIG_DIR=/tmp/demo-config samong vault add payments /tmp/demo
#   SAMONG_CONFIG_DIR=/tmp/demo-config samong-server start --port 8802 --no-open
#   pwsh docs/screenshots.ps1 -Port 8802
param(
  [Parameter(Mandatory = $true)][int]$Port,
  # Relative to this script, so it works from any clone rather than only on the
  # machine it was written on.
  [string]$OutDir = (Join-Path $PSScriptRoot ".")
)

$edge = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
$profileDir = Join-Path $env:TEMP "samong-shot-profile"

function Shot($url, $file, $size) {
  $out = Join-Path $OutDir $file
  if (Test-Path $out) { Remove-Item $out -Force }
  & $edge --headless --disable-gpu --hide-scrollbars `
    --user-data-dir=$profileDir `
    --window-size=$size `
    --virtual-time-budget=9000 `
    --screenshot=$out `
    $url 2>$null | Out-Null
  if (Test-Path $out) {
    $kb = [math]::Round((Get-Item $out).Length / 1KB, 1)
    "OK   $file  ${kb} KB"
  } else {
    "FAIL $file"
  }
}

# The graph is the home surface, so that is the first thing the README shows.
Shot "http://127.0.0.1:$Port/?lang=en" "graph-dark.png" "1440,900"
# Same view in the light theme, to show the theme is real and not a token swap.
Shot "http://127.0.0.1:$Port/?lang=en&theme=light" "graph-light.png" "1440,900"
