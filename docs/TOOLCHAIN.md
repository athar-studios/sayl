# إعدادات سلسلة الأدوات وملاحظات البيئة / Toolchain Setup and Environment Notes

## أوامر الاختبار المعتمدة / Verified Working Test Command

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
cd "<path-to-repo>"
cargo +stable-x86_64-pc-windows-gnu test -p sayl-core
```

## أوامر الفحص المعتمدة / Verified Working Check Command

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
cd "<path-to-repo>"
cargo +stable-x86_64-pc-windows-gnu check -p <crate>
```

## حالة سلاسل الأدوات المحققة / Verified Toolchain Status

- **MSVC Build Tools 14.44.35207:** مثبتة في `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools`.
- **Windows SDK:** تم تثبيت Windows SDK بإصدار `10.0.26100.0` بنجاح (المسار: `C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64\kernel32.lib`).
- **حالة الربط عبر MSVC:** ربط MSVC يعمل بنجاح تام بعد تهيئة بيئة `vcvars64.bat`، وتم حل مشكلة فقدان الحزمة وخطأ `LNK1181` نهائياً. يتم بناء حزم الإنتاج لـ Tauri ومثبتات NSIS و WiX بنجاح.
- **مسار GNU + rust-lld:** تظل سلسلة أدوات `stable-x86_64-pc-windows-gnu` المقترنة برابط `rust-lld` مساراً صالحاً وسريعاً للاختبارات وفحص الأكواد دون اشتراط بيئة MSVC.

## ملاحظات الإنتاج / Production Build Notes

- لبناء حزم الإنتاج الكاملة (`sayl-cli` و `sayl-desktop` ومثبتات Tauri `.exe` و `.msi`)، يُستخدم مسار MSVC مع تهيئة `vcvars64.bat` الرسمية.
- For full production builds, use the active `stable-x86_64-pc-windows-msvc` toolchain inside the MSVC developer environment initialized via `vcvars64.bat`.
