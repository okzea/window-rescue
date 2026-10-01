# Builds dist/WindowRescue.exe: the release binary plus its icon, manifest and version info.
#
#   ./build.ps1             # build into ./dist
#   ./build.ps1 -Install    # build, copy to %LOCALAPPDATA%\Programs\WindowRescue and (re)start it
#
# Resources are written into the finished exe with the Windows UpdateResource API, so the
# build needs nothing but cargo — no resource compiler, no Visual Studio.
param([switch]$Install)
$ErrorActionPreference = 'Stop'

cargo build --release --manifest-path (Join-Path $PSScriptRoot 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }

$dist = Join-Path $PSScriptRoot 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$exe = Join-Path $dist 'WindowRescue.exe'
Copy-Item (Join-Path $PSScriptRoot 'target\release\WindowRescue.exe') $exe -Force

$version = (Select-String -Path (Join-Path $PSScriptRoot 'Cargo.toml') -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value

if (-not ('WindowRescueResources' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

public static class WindowRescueResources
{
    [DllImport("kernel32", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern IntPtr BeginUpdateResource(string file, bool deleteExisting);
    [DllImport("kernel32", SetLastError = true)]
    static extern bool UpdateResource(IntPtr update, IntPtr type, IntPtr name, ushort language, byte[] data, int size);
    [DllImport("kernel32", SetLastError = true)]
    static extern bool EndUpdateResource(IntPtr update, bool discard);

    const int RT_ICON = 3, RT_GROUP_ICON = 14, RT_VERSION = 16, RT_MANIFEST = 24;

    public static void Embed(string exe, byte[] ico, byte[] manifest, string version, string[] strings)
    {
        IntPtr update = BeginUpdateResource(exe, false);
        if (update == IntPtr.Zero) throw new System.ComponentModel.Win32Exception();

        // An .ico file becomes one RT_ICON per image plus an RT_GROUP_ICON directory (id 1).
        int count = BitConverter.ToUInt16(ico, 4);
        var group = new MemoryStream();
        var g = new BinaryWriter(group);
        g.Write((ushort)0); g.Write((ushort)1); g.Write((ushort)count);
        for (int i = 0; i < count; i++)
        {
            int entry = 6 + 16 * i;
            int size = BitConverter.ToInt32(ico, entry + 8);
            int offset = BitConverter.ToInt32(ico, entry + 12);
            var image = new byte[size];
            Array.Copy(ico, offset, image, 0, size);
            Put(update, RT_ICON, i + 1, 0, image);
            g.Write(ico, entry, 12);
            g.Write((ushort)(i + 1));
        }
        Put(update, RT_GROUP_ICON, 1, 0, group.ToArray());
        Put(update, RT_MANIFEST, 1, 0, manifest);
        Put(update, RT_VERSION, 1, 0x0409, VersionInfo(version, strings));

        if (!EndUpdateResource(update, false)) throw new System.ComponentModel.Win32Exception();
    }

    static void Put(IntPtr update, int type, int name, ushort language, byte[] data)
    {
        if (!UpdateResource(update, (IntPtr)type, (IntPtr)name, language, data, data.Length))
            throw new System.ComponentModel.Win32Exception();
    }

    // VS_VERSIONINFO: what Explorer shows under Properties > Details.
    static byte[] VersionInfo(string version, string[] strings)
    {
        var parts = (version + ".0.0.0").Split('.');
        ushort a = ushort.Parse(parts[0]), b = ushort.Parse(parts[1]), c = ushort.Parse(parts[2]);
        var fixedInfo = new MemoryStream();
        var f = new BinaryWriter(fixedInfo);
        f.Write(0xFEEF04BDu); f.Write(0x00010000u);
        for (int i = 0; i < 2; i++) { f.Write((uint)((a << 16) | b)); f.Write((uint)(c << 16)); }
        f.Write(0x3Fu); f.Write(0u); f.Write(0x40004u); f.Write(1u); f.Write(0u); f.Write(0u); f.Write(0u);

        var table = new byte[strings.Length / 2][];
        for (int i = 0; i < table.Length; i++)
        {
            string value = strings[2 * i + 1] + "\0";
            table[i] = Block(strings[2 * i], Encoding.Unicode.GetBytes(value), value.Length, 1);
        }
        var translation = Block("Translation", new byte[] { 0x09, 0x04, 0xB0, 0x04 }, 4, 0);
        return Block("VS_VERSION_INFO", fixedInfo.ToArray(), 52, 0,
            Block("StringFileInfo", null, 0, 1, Block("040904B0", null, 0, 1, table)),
            Block("VarFileInfo", null, 0, 1, translation));
    }

    static byte[] Block(string key, byte[] value, int valueLength, ushort type, params byte[][] children)
    {
        var ms = new MemoryStream();
        var w = new BinaryWriter(ms);
        w.Write((ushort)0); w.Write((ushort)valueLength); w.Write(type);
        w.Write(Encoding.Unicode.GetBytes(key + "\0"));
        Align(ms);
        if (value != null) w.Write(value);
        foreach (var child in children) { Align(ms); w.Write(child); }
        var bytes = ms.ToArray();
        BitConverter.GetBytes((ushort)bytes.Length).CopyTo(bytes, 0);
        return bytes;
    }

    static void Align(MemoryStream ms) { while (ms.Length % 4 != 0) ms.WriteByte(0); }
}
'@
}

$strings = @(
    'CompanyName', 'okzea'
    'FileDescription', 'Window Rescue'
    'FileVersion', $version
    'InternalName', 'WindowRescue'
    'LegalCopyright', 'Copyright (c) 2026 okzea. MIT License.'
    'OriginalFilename', 'WindowRescue.exe'
    'ProductName', 'Window Rescue'
    'ProductVersion', $version
)
$ico = [IO.File]::ReadAllBytes((Join-Path $PSScriptRoot 'assets\WindowRescue.ico'))
$manifest = [IO.File]::ReadAllBytes((Join-Path $PSScriptRoot 'assets\WindowRescue.manifest'))
[WindowRescueResources]::Embed($exe, $ico, $manifest, $version, $strings)
Write-Host "Built $exe ($([math]::Round((Get-Item $exe).Length / 1KB)) KB)"

if (-not $Install) { return }

$dest = Join-Path $env:LOCALAPPDATA 'Programs\WindowRescue'
New-Item -ItemType Directory -Force $dest | Out-Null
Get-Process WindowRescue -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
Copy-Item $exe $dest -Force
Start-Process (Join-Path $dest 'WindowRescue.exe')
Write-Host "Installed to $dest and started."
