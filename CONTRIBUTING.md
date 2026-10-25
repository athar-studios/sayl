# المساهمة في سيل (Contributing to SAYL)

[العربية](#دليل-المساهمة-بالعربية) | [English](#contributing-guide-in-english)

---

## دليل المساهمة (بالعربية)

مرحبًا بك! يسعدنا اهتمامك بالمساهمة في تطوير مشروع **سيل (SAYL)**. سواء كان عملك إصلاح خلل، أو تحسين أداء، أو تدقيق نصوص وترجمة، فنحن نقدّر وقتك وجهدك.

### 1. إعداد بيئة التطوير (Windows)

يعتمد تطوير واختبار سيل على سلسلة أدوات **GNU Toolchain مع رابط `rust-lld`**:

- **Rust:** سلسلة أدوات `stable-x86_64-pc-windows-gnu`.
- **Node.js:** الإصدار 18 أو أحدث مع npm.

#### تجهيز البيئة في PowerShell:
```powershell
# تثبيت سلسلة أدوات GNU
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup component add rustfmt clippy --toolchain stable-x86_64-pc-windows-gnu

# ضبط متغيرات الجلسة
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
```

### 2. فحص الشفرة والاختبارات

قبل إرسال أي تعديل، تأكد من اجتياز كافة الفحوصات:
```powershell
# التحقق من الأنواع وبنية الشفرة
cargo check --workspace

# تشغيل الاختبارات الحتمية (دون الحاجة لاتصال بالإنترنت)
cargo test --workspace

# فحص التنسيق
cargo fmt --all -- --check
```

### 3. أسلوب رسائل الكوميت (Commit Style)

نستخدم صيغة الكوميت الإنجليزية القياسية بصيغة الأمر المباشر (Imperative Mood)، مثل:
- `feat(core): add bandwidth rate limiter`
- `fix(http): handle chunk timeout safely`
- `docs: update cli arguments in user guide`

### 4. إرسال طلبات السحب (Pull Requests)

1. اعمل فرعًا جديدًا يبدأ من `dev` (لا ترسل مباشرة إلى `master`).
2. اكتب اختبارات تغطي وظيفتك الجديدة أو الخلل الذي أصلحته.
3. افتح طلب سحب (PR) موضحًا فيه المشكلة والحل بدقة واختصارات التغييرات.

---

## Contributing Guide (in English)

Thank you for your interest in contributing to **SAYL**. This document outlines our development setup, test execution, commit conventions, and pull request workflow.

### 1. Development Environment

SAYL utilizes the **GNU toolchain with `rust-lld`** for local development and deterministic test execution on Windows:

- **Rust:** `stable-x86_64-pc-windows-gnu` toolchain.
- **Node.js:** v18 or later with npm.

#### PowerShell Session Setup:
```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup component add rustfmt clippy --toolchain stable-x86_64-pc-windows-gnu

$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
```

### 2. Verification & Testing

All HTTP tests execute deterministically against the internal `sayl-test-http` fixture server with zero external network dependencies:

```powershell
# Type check all crates
cargo check --workspace

# Run complete deterministic test suite
cargo test --workspace

# Verify code formatting
cargo fmt --all -- --check
```

### 3. Commit Message Guidelines

Use clear, imperative English commit messages:
- `feat(storage): implement atomic snapshot rotation`
- `fix(bridge): validate origin header length`
- `test(http): add redirect limit test case`

### 4. Pull Request Checklist

1. Always branch off `dev`.
2. Ensure every commit compiles cleanly and all workspace tests pass.
3. Fill out the provided Pull Request template completely.
