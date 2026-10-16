param([Parameter(Mandatory = $true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$target = (Resolve-Path -LiteralPath $Executable).Path
$buildRoot = (Resolve-Path -LiteralPath (Join-Path $projectRoot 'app\src-tauri\target')).Path
if (!$target.StartsWith($buildRoot + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Only generated backend test binaries may be modified.'
}
if (!(('AgSwitchPro.TestResources' -as [type]))) {
    Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
namespace AgSwitchPro {
    public static class TestResources {
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        static extern IntPtr BeginUpdateResource(string file, bool deleteExisting);
        [DllImport("kernel32.dll", SetLastError = true)]
        static extern bool UpdateResource(IntPtr handle, IntPtr type, IntPtr name,
            ushort language, byte[] data, uint size);
        [DllImport("kernel32.dll", SetLastError = true)]
        static extern bool EndUpdateResource(IntPtr handle, bool discard);
        public static void EmbedManifest(string file, byte[] manifest) {
            IntPtr handle = BeginUpdateResource(file, false);
            if (handle == IntPtr.Zero) throw new Win32Exception();
            if (!UpdateResource(handle, new IntPtr(24), new IntPtr(1), 0,
                manifest, (uint)manifest.Length)) {
                int error = Marshal.GetLastWin32Error();
                EndUpdateResource(handle, true);
                throw new Win32Exception(error);
            }
            if (!EndUpdateResource(handle, false)) throw new Win32Exception();
        }
    }
}
'@
}
$manifest = [IO.File]::ReadAllBytes((Join-Path $projectRoot 'app\src-tauri\common-controls.manifest'))
[AgSwitchPro.TestResources]::EmbedManifest($target, $manifest)
