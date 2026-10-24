//! Localized user messages supporting Arabic (default) and English.
//!
//! State names and task IDs remain in Latin script per specification.

use clap::ValueEnum;
use sayl_core::id::TaskId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum Language {
    #[default]
    Ar,
    En,
}

pub fn task_added(id: TaskId, lang: Language) -> String {
    match lang {
        Language::Ar => format!("تمت إضافة المهمة بنجاح: {id}"),
        Language::En => format!("Task added successfully: {id}"),
    }
}

pub fn task_started(id: TaskId, lang: Language) -> String {
    match lang {
        Language::Ar => format!("بدء تحميل المهمة {id}..."),
        Language::En => format!("Starting task {id}..."),
    }
}

pub fn task_paused(id: TaskId, lang: Language) -> String {
    match lang {
        Language::Ar => format!("تم إيقاف المهمة مؤقتاً: {id}"),
        Language::En => format!("Task paused: {id}"),
    }
}

pub fn task_cancelled(id: TaskId, lang: Language) -> String {
    match lang {
        Language::Ar => format!("تم إلغاء المهمة: {id}"),
        Language::En => format!("Task cancelled: {id}"),
    }
}

pub fn task_already_cancelled(id: TaskId, lang: Language) -> String {
    match lang {
        Language::Ar => format!("المهمة ملغاة بالفعل: {id}"),
        Language::En => format!("Task is already cancelled: {id}"),
    }
}

pub fn task_removed(id: TaskId, lang: Language) -> String {
    match lang {
        Language::Ar => format!("تم حذف المهمة: {id}"),
        Language::En => format!("Task removed: {id}"),
    }
}

pub fn task_not_found(prefix: &str, lang: Language) -> String {
    match lang {
        Language::Ar => format!("خطأ: لم يتم العثور على مهمة بالمعرّف أو البادئة '{prefix}'"),
        Language::En => format!("Error: No task found matching prefix '{prefix}'"),
    }
}

pub fn ambiguous_prefix(prefix: &str, matches: &[String], lang: Language) -> String {
    let list = matches.join(", ");
    match lang {
        Language::Ar => format!("خطأ: البادئة '{prefix}' غامضة وتطابق عدة مهام: [{list}]"),
        Language::En => {
            format!("Error: Ambiguous prefix '{prefix}' matches multiple tasks: [{list}]")
        }
    }
}

pub fn task_failed(id: TaskId, error: &str, lang: Language) -> String {
    match lang {
        Language::Ar => format!("فشلت المهمة {id}: {error}"),
        Language::En => format!("Task {id} failed: {error}"),
    }
}

pub fn task_completed(id: TaskId, lang: Language) -> String {
    match lang {
        Language::Ar => format!("اكتمل تحميل المهمة بنجاح: {id}"),
        Language::En => format!("Task completed successfully: {id}"),
    }
}

pub fn recover_summary(count: usize, lang: Language) -> String {
    match lang {
        Language::Ar => format!("تم استرداد {count} مهام بنجاح."),
        Language::En => format!("Recovered {count} tasks successfully."),
    }
}

pub fn no_tasks_to_recover(lang: Language) -> String {
    match lang {
        Language::Ar => "لا توجد مهام تحتاج إلى استرداد.".to_string(),
        Language::En => "No tasks require recovery.".to_string(),
    }
}

pub fn empty_task_list(lang: Language) -> String {
    match lang {
        Language::Ar => "لا توجد مهام تنزيل مسجلة.".to_string(),
        Language::En => "No download tasks found.".to_string(),
    }
}

pub fn err_connections_zero(lang: Language) -> String {
    match lang {
        Language::Ar => "خطأ في الاستخدام: يجب أن تكون قيمة --connections أكبر من صفر.".to_string(),
        Language::En => "Usage error: --connections must be greater than zero.".to_string(),
    }
}

pub fn err_concurrent_zero(lang: Language) -> String {
    match lang {
        Language::Ar => "خطأ في الاستخدام: يجب أن تكون قيمة --concurrent أكبر من صفر.".to_string(),
        Language::En => "Usage error: --concurrent must be greater than zero.".to_string(),
    }
}

pub fn all_queued_started(count: usize, lang: Language) -> String {
    match lang {
        Language::Ar => format!("تم الانتهاء من تشغيل {count} مهام كانت في قائمة الانتظار."),
        Language::En => format!("Finished running {count} queued tasks."),
    }
}
