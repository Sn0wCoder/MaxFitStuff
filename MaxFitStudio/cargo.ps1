
#   .\cargo.ps1 build
#   .\cargo.ps1 run -p maxfit-gui
#   .\cargo.ps1 test

$rustup = "$env:USERPROFILE\.cargo\bin\rustup.exe"
$env:RUSTC   = (& $rustup which rustc).Trim()
$env:RUSTDOC = (& $rustup which rustdoc).Trim()
& "$env:USERPROFILE\.cargo\bin\cargo.exe" @args
exit $LASTEXITCODE
