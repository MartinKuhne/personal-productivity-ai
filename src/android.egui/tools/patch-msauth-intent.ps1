# tools/patch-msauth-intent.ps1
#
# Post-build patch for the FastMD egui Android APK: adds the
# `msauth://com.fastmd.android.egui` deep-link intent filter to the
# generated AndroidManifest.xml and re-signs the APK with the debug
# keystore so `adb install` accepts it.
#
# Why this exists: cargo-apk 0.10 only emits the `MAIN`/`LAUNCHER`
# intent filter in its generated manifest, and the `[[package.metadata.android.activity.intent_filter]]`
# TOML hook is parsed but not applied. Once the JNI deep-link poller
# in `src/android.rs` needs the `msauth://` URI to be routable back to
# the app, the manifest needs this second filter.
#
# Usage:
#   pwsh tools/patch-msauth-intent.ps1
#   pwsh tools/patch-msauth-intent.ps1 -ApkPath target\debug\apk\fastmd-android-egui.apk
#
# Side effect: the script writes the patched APK next to the input
# (`fastmd-android-egui-patched.apk`) and the original is preserved.

[CmdletBinding()]
param(
    [string]$ApkPath = "target\debug\apk\fastmd-android-egui.apk",
    [string]$Package  = "com.fastmd.android.egui",
    [string]$Scheme   = "msauth"
)

$ErrorActionPreference = 'Stop'

# Make the .NET ZipFile API available for the extract + splice steps.
Add-Type -AssemblyName System.IO.Compression.FileSystem

# ---------------------------------------------------------------------
# Resolve toolchain paths
# ---------------------------------------------------------------------
if (-not $env:ANDROID_HOME) {
    $env:ANDROID_HOME = 'C:\Users\mkuhn\AppData\Local\Android\Sdk'
}
if (-not (Test-Path $env:ANDROID_HOME)) {
    throw "ANDROID_HOME=$($env:ANDROID_HOME) does not exist; set it or pass -AndroidHome."
}

$BuildTools = Get-ChildItem -Path (Join-Path $env:ANDROID_HOME 'build-tools') -Directory `
    | Sort-Object Name -Descending | Select-Object -First 1
$Aapt2       = Join-Path $BuildTools.FullName 'aapt2.exe'
# apksigner ships as both a .bat wrapper and a lib/apksigner.jar. The
# .bat wrapper prints a benign JVM "restricted method" warning on
# modern JDKs that PowerShell's $ErrorActionPreference='Stop' turns
# into a terminating error. We invoke the JAR directly via
# `java -jar` to bypass the wrapper and get a clean exit code.
$ApksignerJar = Join-Path $BuildTools.FullName 'lib\apksigner.jar'
$Java         = (Get-Command java -ErrorAction SilentlyContinue).Source
$AndroidJar  = Get-ChildItem -Path (Join-Path $env:ANDROID_HOME 'platforms') -Directory `
    | Sort-Object Name -Descending | Select-Object -First 1
$AndroidJarPath = Join-Path $AndroidJar.FullName 'android.jar'

foreach ($tool in @($Aapt2, $ApksignerJar, $AndroidJarPath, $Java)) {
    if (-not (Test-Path $tool) -and $tool -ne $Java) {
        throw "Required tool not found: $tool"
    }
}
if (-not $Java) {
    throw "java.exe not on PATH; set JAVA_HOME or add it to PATH."
}

# ---------------------------------------------------------------------
# Resolve input/output paths
# ---------------------------------------------------------------------
$ApkPath = (Resolve-Path $ApkPath).Path
$Dir     = Split-Path -Parent $ApkPath
$Name    = [System.IO.Path]::GetFileNameWithoutExtension($ApkPath)
$OutApk  = Join-Path $Dir "$Name-patched.apk"

Write-Host "[patch-msauth] input : $ApkPath"
Write-Host "[patch-msauth] output: $OutApk"

# ---------------------------------------------------------------------
# Build a complete AndroidManifest.xml (text) with both intent filters,
# then use aapt2 link to compile it into a binary AXML. The link step
# also produces a small "manifest-only" APK that we extract the AXML
# from and splice into the original APK.
# ---------------------------------------------------------------------

$RealManifest = @"
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
          package="$Package"
          android:versionCode="1"
          android:versionName="0.1.0">
    <uses-sdk android:minSdkVersion="26" android:targetSdkVersion="34"/>
    <uses-permission android:name="android.permission.INTERNET"/>
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE"/>
    <application android:label="FastMD egui" android:hasCode="false" android:debuggable="true">
        <activity android:name="android.app.NativeActivity" android:exported="true" android:configChanges="orientation|keyboardHidden|screenSize">
            <meta-data android:name="android.app.lib_name" android:value="fastmd_android_egui"/>
            <intent-filter>
                <action android:name="android.intent.action.MAIN"/>
                <category android:name="android.intent.category.LAUNCHER"/>
            </intent-filter>
            <intent-filter>
                <action android:name="android.intent.action.VIEW"/>
                <category android:name="android.intent.category.DEFAULT"/>
                <category android:name="android.intent.category.BROWSABLE"/>
                <data android:scheme="$Scheme" android:host="$Package"/>
            </intent-filter>
        </activity>
    </application>
</manifest>
"@

$RealManifestPath = Join-Path $env:TEMP "fastmd-egui-manifest-$([System.Guid]::NewGuid()).xml"
$RealManifest | Out-File -FilePath $RealManifestPath -Encoding UTF8

# aapt2 link builds an APK from the text manifest. We then extract
# just the AXML manifest from that APK and splice it into the original.
$NewManifestApk = Join-Path $env:TEMP "fastmd-egui-newmanifest-$([System.Guid]::NewGuid()).apk"
$LinkArgs = @(
    'link'
    '-o', $NewManifestApk
    '-I', $AndroidJarPath
    '--manifest', $RealManifestPath
)
& $Aapt2 @LinkArgs 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "aapt2 link failed (exit $LASTEXITCODE)"
}
if (-not (Test-Path $NewManifestApk)) {
    throw "aapt2 link did not produce $NewManifestApk"
}

# Extract the compiled AXML manifest from the new APK using the .NET
# ZipFile API. Expand-Archive refuses .apk extensions, so we go
# through the same path we use for splicing below.
$ExtractDir = Join-Path $env:TEMP "fastmd-egui-extract-$([System.Guid]::NewGuid())"
New-Item -ItemType Directory -Path $ExtractDir | Out-Null
$extractZip = [System.IO.Compression.ZipFile]::OpenRead($NewManifestApk)
foreach ($entry in $extractZip.Entries) {
    if ($entry.FullName.TrimStart('/') -eq 'AndroidManifest.xml') {
        $outPath = Join-Path $ExtractDir 'AndroidManifest.xml'
        $s = $entry.Open()
        $fs = [System.IO.File]::Create($outPath)
        $s.CopyTo($fs)
        $fs.Close()
        $s.Close()
        break
    }
}
$extractZip.Dispose()
$CompiledManifest = Join-Path $ExtractDir 'AndroidManifest.xml'
if (-not (Test-Path $CompiledManifest)) {
    throw "Compiled manifest not found at $CompiledManifest"
}

# ---------------------------------------------------------------------
# Splice the new manifest into the original APK
# ---------------------------------------------------------------------
# Use .NET's ZipArchive so we don't depend on any external zip tool.
$OutTmp = Join-Path $env:TEMP "fastmd-egui-out-$([System.Guid]::NewGuid()).apk"

$src = [System.IO.Compression.ZipFile]::OpenRead($ApkPath)
$dst = [System.IO.Compression.ZipFile]::Open($OutTmp, 'Create')

# Files we are replacing wholesale.
$Replaced = New-Object 'System.Collections.Generic.HashSet[string]'
$Replaced.Add('AndroidManifest.xml') | Out-Null

foreach ($entry in $src.Entries) {
    $name = $entry.FullName.TrimStart('/')
    if ($Replaced.Contains($name)) { continue }

    $out = $dst.CreateEntry($entry.FullName)
    $outStream = $out.Open()
    $entryStream = $entry.Open()
    $entryStream.CopyTo($outStream)
    $outStream.Close()
    $entryStream.Close()
}

# Add the new manifest.
$out = $dst.CreateEntry('AndroidManifest.xml')
$outStream = $out.Open()
$manifestStream = [System.IO.File]::OpenRead($CompiledManifest)
$manifestStream.CopyTo($outStream)
$outStream.Close()
$manifestStream.Close()

$dst.Dispose()
$src.Dispose()

Move-Item -Force $OutTmp $OutApk

# ---------------------------------------------------------------------
# Re-sign with the debug keystore (or a user-provided one)
# ---------------------------------------------------------------------
# Find the debug keystore that `cargo apk` would have used.
$DebugKs = Get-ChildItem -Path . -Filter 'debug.keystore' -ErrorAction SilentlyContinue `
    | Select-Object -First 1
$KsPath = if ($DebugKs) { $DebugKs.FullName } else { "$env:USERPROFILE\.android\debug.keystore" }
$KsPass = 'android' # standard debug keystore password
$KeyAlias = 'androiddebugkey'

if (Test-Path $KsPath) {
    Write-Host "[patch-msauth] signing with $KsPath"
    # Invoke the apksigner JAR directly so we don't hit the .bat
    # wrapper's JVM "restricted method" warning.
    $SignArgs = @(
        '-jar', $ApksignerJar
        'sign'
        '--ks', $KsPath
        '--ks-pass', "pass:$KsPass"
        '--ks-key-alias', $KeyAlias
        '--key-pass', "pass:$KsPass"
        $OutApk
    )
    # The Java 24+ launcher (java.exe) writes a one-line "restricted
    # method" warning to stderr on Windows. It's harmless and we don't
    # want it to be promoted into a terminating error by
    # $ErrorActionPreference=Stop. So we drop to 'Continue' for just
    # the sign call, then restore the strict mode.
    $prevPref = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $outFile = [System.IO.Path]::GetTempFileName()
    $errFile = [System.IO.Path]::GetTempFileName()
    & $Java @SignArgs 1>$outFile 2>$errFile | Out-Null
    $signExit = $LASTEXITCODE
    $ErrorActionPreference = $prevPref
    $idsig = "$OutApk.idsig"
    if ($signExit -ne 0 -and -not (Test-Path $idsig)) {
        Get-Content $errFile
        Get-Content $outFile
        throw "apksigner sign failed (exit $signExit)"
    }
    if (Test-Path $idsig) {
        Write-Host "[patch-msauth] signed: $idsig"
    }
    Remove-Item -Force $outFile, $errFile -ErrorAction SilentlyContinue
} else {
    Write-Host "[patch-msauth] WARNING: no debug keystore at $KsPath." -ForegroundColor Yellow
    Write-Host "[patch-msauth] The patched APK is unsigned; sign it manually before installing." -ForegroundColor Yellow
}

# Cleanup
Remove-Item -Recurse -Force $ExtractDir -ErrorAction SilentlyContinue
Remove-Item -Force $NewManifestApk -ErrorAction SilentlyContinue
Remove-Item -Force $RealManifestPath -ErrorAction SilentlyContinue

Write-Host "[patch-msauth] done: $OutApk" -ForegroundColor Green
Write-Host "[patch-msauth] install with:  adb install $OutApk"
