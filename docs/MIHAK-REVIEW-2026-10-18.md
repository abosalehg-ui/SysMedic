# مِحَك — مراجعة مستودع SysMedic (الجولة الثالثة)

> **النوع المكتشف:** تطبيق (Application) — أداة تشخيص لنظام لينكس بواجهتين CLI و GTK4/libadwaita. ليست لعبة، ولا فيها حلقة رسم أو فيزياء.
> **تاريخ المراجعة:** 2026-10-18 — على أساس الـ commit `16f4ff6` (فرع `main`، بعد دمج PR #35).
> **الدرجة الإجمالية المرجّحة: 8.2 / 10**

---

## 1. نظرة عامة سريعة

SysMedic "طبيب" لنظام لينكس: 16 مجمّعاً (collector) يقرؤون `/proc` و`/sys` وأدوات النظام بالتوازي، ثم 30 قاعدة تشخيص نقية (pure rules) تُخرج درجة صحية من 100 مع نسبة تغطية، وشرحاً لكل نتيجة بالعربية والإنجليزية بلا إنترنت، وستة إصلاحات بمعاينة/تأكيد/تراجع تُنفَّذ كـ root عبر أداتين مميّزتين (privileged helpers) مفصولتين بإجراءي polkit.

البنية Cargo workspace من 11 crate: `core` (النموذج والدرجات والمسارات) → `collectors` (كل الـ I/O) → `diagnostics` (قواعد نقية) → `knowledge` (YAML مضمّن + مزوّد LLM اختياري) → `fixes` + `fix-helper` → `report` / `history` / `diskscan` → `cli` + `gui`.

**التقنيات:** Rust 2021 (MSRV معلن 1.87، بُني هنا على 1.97.0)، GTK4 0.9 + libadwaita 0.7، `clap 4`، `serde` / `serde_json` / `serde_yaml_ng 0.10`، `thiserror 2`، `anyhow`، `humantime`، `owo-colors 4`، `ureq 2` (rustls) للمكالمة الاختيارية مع Claude API. التغليف: deb (مُتحقَّق في CI) وFlatpak/AppImage/Snap (manifests). CI: fmt + clippy `-D warnings` + tests + coverage + MSRV + `rustsec/audit-check`، والـ Actions كلها مثبّتة على SHA.

**حجم المستودع:** 105 ملفات خارج `target/` و`.git`، منها 53 ملف Rust بـ ~14,400 سطر (شاملة الاختبارات)، و~480 سطر YAML لقاعدة المعرفة، وصفحتا موقع (en/ar)، وتسعة مستندات في `docs/`.

**نطاق المراجعة:** قُرئت كل ملفات `crates/**/*.rs` كاملة، وملفات الإعداد والتغليف وسير عمل CI/Release/Pages، وسياسة polkit، ورأس `knowledge.yaml`، وصفحة الموقع العربية. شُغِّلت الاختبارات فعلياً: `cargo test --workspace --locked --exclude sysmedic-gui` ⇐ **202 اختباراً ناجحاً، 0 فشل** (زائد 22 في crate الواجهة لم تُشغَّل لغياب مكتبات GTK4 في بيئة المراجعة — المجموع 224 كما يعلن المستودع). `cargo clippy --all-targets -D warnings` ⇐ نظيف، `cargo fmt --check` ⇐ نظيف. أي حكم على *سلوك* الواجهة الرسومية أدناه مبني على قراءة الكود لا على تشغيله.

**سياق مهم:** هذه الجولة الثالثة. الجولتان السابقتان (`MIHAK-REVIEW-2026-08-18.md` و`MIHAK-REVIEW-2026-09-19.md`) خرجتا بـ 39 بنداً نُفِّذت **كلها** — تحققتُ من كل بند في الكود الحالي: استثناء SSH في `fix.enable_ufw`، `firewall_frontend`، صلاحيات الـ journal `0644`، التراجع من الواجهة، `classify_failure` + stderr، `needs_collectors`، مهلة `RealRunner`، التعريب الشامل للطرفية، `classify_port`، SHA256SUMS + provenance، MSRV، توحيد حزم الألوان، إلخ. لذلك هذه المراجعة **لا تكرر** ما عولج، وتركّز على ما ظهر *بسبب* الإصلاحات الجديدة أو ما لم يُلتفت إليه. الدرجة ثابتة عند 8.2 لا لأن المشروع ما تحسّن — تحسّن فعلاً — بل لأن هذه الجولة كشفت مساراً جديداً لنفس خطر "الإصلاح الآمن يقفلك خارج جهازك" (4.2)، ومجموعة فجوات RTL في أحدث كود للواجهة.

---

## 2. المراجعة التقنية والهندسية (45%)

### 2.1 البنية المعمارية وفصل المسؤوليات — 9/10

الحدود مفروضة بالنوع لا بالاتفاق: `Diagnostic` ممنوعة من الـ I/O، كل الـ I/O في `collectors`، والـ GUI مقسوم إلى `viewmodel` (بلا أنواع GTK، مختبر بالوحدة) و`ui`. الحدّ الأمني نظيف: الأداة المميّزة تقبل معرّفاً فقط، وتعيد بناء الخطة من `needs_collectors()`، وتفصل الطبقتين Routine/Destructive بالكود وباختبار.

الخصم الوحيد: **عقد "الخطة نفسها تقود المعاينة والتنفيذ" ليس مفروضاً عبر حدّ الصلاحيات.** `ARCHITECTURE.md` يقول *"the same plan drives both the preview and the run"*، لكن الواقع في `gui/ui.rs` → `confirm_and_apply` و`fix-helper/src/lib.rs` → `snapshot_for`:

```rust
// الواجهة: خطة مبنية من لقطة غير مميّزة وقت الفحص
confirm_and_apply(&window, lang, &plan, &toasts, on_changed.clone());
// ...
// الأداة المميّزة: خطة مبنية من لقطة root جديدة وقت التنفيذ
let snapshot = snapshot_for(&fix_id);
let plan = plan(&fix_id, &snapshot)
```

ما وافق عليه المستخدم (خطة A) وما نُفِّذ (خطة B) شيئان مبنيان من لقطتين مختلفتين بصلاحيتين مختلفتين. تفصيل الأثر الأمني في 4.2. هندسياً الحل بسيط ولا يوسّع سطح الهجوم: بصمة (fingerprint) للأوامر.

### 2.2 قابلية القراءة والصيانة — 9/10

التعليقات تشرح "ليش"، الدوال قصيرة، الأسماء واضحة، و`CONTRIBUTING.md` يوثّق القواعد التي تحرسها الاختبارات. الملاحظة من الجولة السابقة ما زالت قائمة وتتراكم: تعليقات بصيغة سرد تاريخي ("The old `undo` hardcoded…"، "`.status()` threw them away"، "This used to fall back to `/tmp`") في `fixes.rs` و`ui.rs` و`history.rs` و`paths.rs` — مكانها CHANGELOG لا الكود.

### 2.3 التكرار (DRY) و Code Smells — 8/10

**(أ) استدعاء pkexec مكرّر مرتين في `gui/ui.rs`.** `run_helper()` موجودة:

```rust
fn run_helper(helper: String, args: Vec<&'static str>) -> Result<(), (HelperFailure, String)> {
    match Command::new("pkexec").arg(&helper).args(&args).output() { ... }
}
```

لكن `confirm_and_apply` تعيد كتابة نفس الـ `match` حرفياً داخل `spawn_blocking` لأن توقيع `run_helper` يقبل `Vec<&'static str>` فقط ولا يقبل `fix_id` الديناميكي. **الحل:** `args: Vec<String>` واستدعِها من الموضعين.

**(ب) `FixOutcome` / `fix_outcome()` صارت بقايا.** بعد إضافة `HelperFailure` في الجولة السابقة، `fix_outcome(result.is_ok())` تلفّ `bool` في enum ثم تُقارن بـ `== FixOutcome::Applied` — طبقة بلا معلومة. والاختبار `a_failed_helper_is_never_reported_as_success` يختبر هذه اللفّة لا السلوك. **الحل:** احذف `FixOutcome` واستعمل `match result`.

**(ج) `fixes::undo_commands()` واجهة عامة لا يستدعيها أحد خارج اختبارها.** بعد أن صار `undo()` يستدعي `fix.undo_from(restore)` مباشرة، الدالة بلا مستهلك. احذفها أو اجعلها `#[cfg(test)]`.

### 2.4 معالجة الأخطاء والحالات الحدّية — 8/10

الأساس ممتاز (أخطاء مصنّفة، write-ahead journal، لا `unwrap` في مسارات الإنتاج — تحققت: كل الـ `unwrap`/`expect` خارج `#[cfg(test)]` إمّا على `serde` لأنواع تُسلسَل حتماً أو `unreachable!` محروس). ثلاث حالات حدّية ملموسة:

**(أ) نص حوار الفشل يقول عكس ما حدث.** في `gui/viewmodel.rs` → `failure_body`:

```rust
let reason = match failure {
    HelperFailure::Cancelled => strings.fix_cancelled_body,
    HelperFailure::NotAuthorized => strings.fix_unauthorized_body,
    HelperFailure::Failed => strings.fix_failed_body,   // ←
};
```

و`fix_failed_body` = *"Authorization was cancelled or failed, or the fix helper is not installed."* — وهي الجملة العامة القديمة. لكن `HelperFailure::Failed` تعني بالتعريف (تعليق الـ enum نفسه): *"The helper ran as root and failed"* — أي أن المصادقة **نجحت**. المستخدم الذي مات عنده `apt-get` على قفل dpkg يقرأ "أُلغيت المصادقة أو فشلت" ثم تحته ذيل stderr يناقضها. والاختبار `a_failed_fix_shows_what_the_helper_said` يثبّت هذا النص بـ `assert_eq!(bare, fix_failed_body)`. **الحل:** نص جديد: *"The fix ran but did not complete. The helper reported:"* / *"شُغِّل الإصلاح لكنه لم يكتمل. قالت أداة الإصلاح:"* وحدّث الاختبار.

**(ب) journal تالف = لا إصلاحات أبداً بلا مخرج.** `fix-helper/src/lib.rs` → `open_journal` → `Journal::load` يرجع `JournalError::Corrupt` لأي JSON غير صالح في `/var/lib/sysmedic/journal.json`، فيرفض الـ helper **كل** `apply` و`undo`. الكتابة atomic فالتلف نادر، لكن إن وقع (تعديل يدوي، استعادة نسخة احتياطية مبتورة) فلا طريق للتعافي إلا حذف الملف يدوياً كـ root، والواجهة تعرض "The fix was not applied" مع رسالة `corrupt journal at …` لا تقول ماذا يفعل. **الحل:** في `Journal::load`، عند `Corrupt` انقل الملف إلى `journal.json.corrupt-<timestamp>` وابدأ بسجل فارغ مع سطر في stderr — سجل تدقيق لا يُفقد، والإصلاحات لا تُعطَّل.

**(ج) رمز الخروج 1 يعني شيئين.** في `cli/main.rs`:

```rust
pub const DEGRADED: u8 = 1;   // --exit-code: أسوأ نتيجة High
// ...
Err(e) => { eprintln!("sysmedic: {e:#}"); std::process::ExitCode::FAILURE }  // = 1 أيضاً
```

سكربت مراقبة يفحص `$? == 1` لا يفرّق بين "النظام متدهور" و"sysmedic انهار". **الحل:** `ExitCode::from(3)` للأخطاء، ووثّق الثلاثة في `--help`.

### 2.5 الأداء واستهلاك الموارد — 8.5/10

المجمّعات متوازية، فحص القرص كسول وقابل للإلغاء، الـ registry `LazyLock`، الـ helper يجمع ما يحتاجه فقط. ملاحظة واحدة صغيرة: `gui/ui.rs` → `report_view` يقرأ ملف التاريخ كاملاً (`sysmedic_history::load`) عند كل عرض تقرير، و`run_engine` يقرؤه مرة ثانية في `append_throttled` — قراءتان لكل فحص. أثرها عملياً معدوم (ملف بضعة كيلوبايتات)، تُذكر للاكتمال.

### 2.6 الاختبارات — 8/10

224 اختباراً حقيقياً، والحرّاس الذكيون كثر (`the_entry_is_journalled_before_any_command_runs`، التفكيك الحصري لـ `Strings`، `every_finding_has_arabic_title_and_summary_templates`، `fix_collector_names_are_real`).

الخصم في نوع واحد من الفجوة: **اختبارات تثبّت سلوكاً خاطئاً** (ثلاثة مواضع)، واختبارات على فيكستشر متفائل تفوّت الحالة الواقعية:
- `viewmodel.rs` → `a_failed_fix_shows_what_the_helper_said` يثبّت النص الخاطئ في 2.4(أ).
- `core/alert.rs` → `alert_text_is_localized` يؤكد `ar[0].body.contains('%')` بينما سياسة الطباعة العربية الموثّقة في رأس `knowledge.yaml` تقول ٪ (U+066A) حصراً (3.4 د).
- `collectors/flatpak.rs` → `identifies_unused_runtime` فيكستشره منصّتان أساسيتان فقط، بلا أي امتداد (`.Locale`، `.GL.default`) — وهو ما يُظهر الخطأ في 4.2(ب).
- `fixes.rs` → اختبارات `fix.enable_ufw` كلها على المنفذ 22؛ لا اختبار لـ `Port 2222` (4.2 أ).

**الحل:** أضف الأربعة كاختبارات فشل أولاً ثم أصلح الكود.

### 2.7 التوثيق — 8.5/10

README وARCHITECTURE وCHANGELOG وSECURITY وCONTRIBUTING على مستوى ممتاز. تناقضات صغيرة تهزّ الثقة:
- `docs/ARCHITECTURE.md` يقول الافتراضي `claude-opus-4-8` بينما الكود `claude-opus-5` (و`CHANGELOG` يوثّق التغيير). ونفس الملف يقول *"The resident `sysmedicd` service arrives in M5"* بينما M5 نُفِّذ بمؤقّتات systemd بلا daemon — الجملة التي فوقها بسطرين تقول ذلك.
- `INSTALL.md` يمثّل بـ `SYSMEDIC_LLM_MODEL=claude-sonnet-5` — الجيل الحالي `claude-sonnet-5-5`. والافتراضي في `llm.rs` (`claude-opus-5`) صالح لكن `claude-opus-5-5` هو Opus الحالي **وأرخص** ($4/$20 مقابل $5/$25 لكل مليون رمز) بنفس الطبقة. تحقّقت من الاثنين من مرجع API الحالي.
- `history.rs` تعليق `append`: *"Owner-only, consistent with the journal"* — لم يعد متسقاً بعد أن صار الـ journal `0644` عمداً في الجولة السابقة.
- `metainfo.xml` يعلن إصدار 0.2.0 بتاريخ 2026-07-26، و`CHANGELOG` يقارن `[Unreleased]` بـ `v0.1.0...HEAD`، و`git tag` يرجع **لا شيء** — أي أن سير عمل Release (المشغَّل بالوسم) لم يُشغَّل قط، وSHA256SUMS والـ provenance اللذان أُضيفا في الجولة السابقة لم يُختبَرا فعلياً.

### 2.8 اتفاقيات التسمية ونمط موحّد — 9/10

`fmt` و`clippy -D warnings` مفروضان. المعرّفات محروسة باختبار. ملاحظة طباعية واحدة: تنوين الفتح يُكتب بأسلوبين في نفس المنتج — `viewmodel.rs` "سليمًا" (الألف بعد التنوين) و`text.rs` "سليماً" (التنوين على الألف). اختر واحداً ووثّقه في سياسة `knowledge.yaml`.

### **متوسط القسم الهندسي: 8.5 / 10**

(9 + 9 + 8 + 8 + 8.5 + 8 + 8.5 + 9) ÷ 8 = 8.5

---

## 3. المظهر والتصميم وتجربة المستخدم (30%)

للمشروع واجهتان، فالمعيار طُبّق على الاثنتين: معايير الواجهة الرسومية على GTK، ومعايير الطرفية (وضوح المخرج، رسائل الخطأ، `--help`) على الـ CLI.

### 3.1 الاتساق البصري — 8.5/10

Adwaita بلا اختراع، CSS مخصص سطران، لوحة الخريطة الشجرية من ألوان GNOME مع نص محسوب بالإضاءة النسبية، وحزم الألوان الآن مطابقة لحزم التقدير باختبار. تقرير HTML يحترم `prefers-color-scheme` و`prefers-reduced-motion`. لا ملاحظات جديدة.

### 3.2 سهولة الاستخدام والتنقل — 7.5/10

المهمة الأساسية بنقرة صفر، والتراجع صار متاحاً من الواجهة. ثلاثة خصومات:

**(أ) إصلاح يظل معروضاً إلى الأبد بعد تطبيقه.** `fixes.rs` → `SnapRetain::plan`:

```rust
let snap = s.snap.as_ref()?;
if snap.disabled_revisions == 0 { return None; }
```

الإصلاح يضبط `refresh.retain=2`، و2 تعني "النسخة الحالية + نسخة سابقة معطّلة". فبعد تطبيقه كل snap حُدِّث مرة يحتفظ بنسخة `disabled` واحدة ⇒ `disabled_revisions > 0` ⇒ القاعدة `storage.snap_old_revisions` تطلق (خصم 5 نقاط دائم على كل Ubuntu)، وزر "تطبيق الإصلاح" يبقى، والضغط عليه يطلب كلمة المرور ويشغّل `snap set system refresh.retain=2` بلا أثر ويُسجَّل مدخلة قابلة للتراجع في الـ journal — في كل مرة. **الحل:** في `plan` أرجع `None` إذا `snap.refresh_retain <= Some(TARGET_RETAIN)`؛ وفي `collectors/snap.rs` → `parse_disabled_revisions` عدّ النسخ المعطّلة **لكل snap** وأطلق القاعدة فقط لمن عنده ≥ 2 (أي ما يتجاوز ما يحتفظ به retain=2).

**(ب) التراجع يأخذ موافقة أضعف من التطبيق.** حوار التطبيق يعرض الأوامر والمسارات والخطورة؛ حوار التراجع في `gui/ui.rs` → `install_undo_action` يعرض **العنوان فقط** (`adw::AlertDialog::new(Some(strings.undo_confirm_title), Some(&title))`) — لا متى طُبّق، ولا ما سيُنفَّذ (`ufw disable` + `ufw delete allow 22/tcp` بعد أسابيع من تفعيل الجدار الناري وإضافة قواعد عليه). والـ CLI مثله (`would_undo` + المعرّف). **الحل:** أضف `applied_at` (بالوقت المحلي) وقائمة `fix.undo_from(entry.restore)` في الحوار، كما تفعل المعاينة.

**(ج) زر "تطبيق الإصلاح" يظهر داخل Flatpak حيث لا يمكن أن يعمل.** `INSTALL.md` و`packaging/README.md` يقولان بوضوح إن الإصلاحات تحتاج الـ `.deb`؛ لكن `report_view` يعرض الزر دائماً، فمستخدم Flathub يضغطه ويقرأ "Authorization was refused, or the fix helper is not installed". **الحل:** إذا وُجد `/.flatpak-info` اعرض بدل الزر سطراً "الإصلاحات متاحة في حزمة .deb" (`viewmodel::fixes_available()` نقية ومختبَرة).

### 3.3 الاستجابة — 9/10

`width_request(360)` و`Breakpoint` على 550sp، وتقرير HTML بـ `max-width: 860px`. لا ملاحظات.

### 3.4 العربية و RTL — 7/10

ما سبق إصلاحه صامد (اتجاه الواجهة، عزل bidi في القوالب، الشارات، انعكاس الخريطة، `--lang` عام). لكن أحدث كود للواجهة أعاد نفس فئة الأخطاء التي عولجت في القوالب:

**(أ) أسماء المجلدات العربية في الخريطة الشجرية تُرسم بحروف مفكّكة معكوسة.** `gui/disk.rs` → `draw_treemap`:

```rust
cr.select_font_face("sans-serif", FontSlant::Normal, FontWeight::Bold);
cr.set_font_size(12.0);
cr.move_to(tile.rect.x + 6.0, tile.rect.y + 18.0);
let _ = cr.show_text(&tile.label);
```

`cairo::Context::show_text` هي "toy text API" في cairo: بلا تشكيل حروف (shaping) ولا bidi. على Ubuntu بلغة عربية مجلدات XDG الافتراضية اسمها `المستندات`، `التنزيلات`، `الصور`، `الفيديو` — وهي بالضبط أكبر مجلدات البيت التي تملأ الخريطة — فتظهر كـ "ت ا د ن ت س م ل ا" بحروف منفصلة من اليسار. أيضاً النص لا يُقصّ (clip) على حدود البلاطة، فاسم طويل يمتد فوق البلاطة المجاورة. **الحل:** استعمل Pango: `widget.create_pango_layout(Some(&tile.label))` + `layout.set_width(((tile.rect.w - 12.0) * pango::SCALE as f64) as i32)` + `set_ellipsize(pango::EllipsizeMode::End)` ثم `pangocairo::functions::show_layout(cr, &layout)` (أضف `pangocairo = "0.20"` إلى `Cargo.toml`؛ gtk4 يعتمد عليه أصلاً فلا اعتمادية جديدة فعلياً).

**(ب) الشريط الزمني (sparkline) معكوس في العربية وتلميحه يقول العكس.** `gui/ui.rs` → `report_view`:

```rust
let history = gtk::Label::new(Some(&format!("{spark}{trend}")));
history.set_tooltip_text(Some(strings.history_tooltip));  // "الأحدث على اليمين"
```

حروف الشريط (U+2581–U+2588) محايدة الاتجاه؛ أول حرف قوي في النص عربي (في `trend`) فتصير الفقرة RTL ويُعرض الشريط من اليمين إلى اليسار — الأحدث على **اليسار**. التلميح `history_tooltip` يقول "الأحدث على اليمين". **الحل:** `format!("\u{2066}{spark}\u{2069}{trend}")` (LRI…PDI) ليبقى المحور الزمني من اليسار إلى اليمين كما يقول التلميح.

**(ج) المسارات غير معزولة bidi في الواجهة والتنبيهات.** `lang::isolate` أُضيفت للقوالب تحديداً لهذه المشكلة، لكن:
- `gui/disk.rs` → `scanning_label`: `format!("{} {}…", strings.disk_scanning_path, path.display())` ⇒ "جارٍ فحص /home/user/…" بشرطة مائلة أولى منفصلة.
- `gui/disk.rs` بعد الفحص: `format!("{prefix}{} — {}", shown.display(), human_size(node.size))`.
- `gui/ui.rs` → تصدير: `format!("{} — {}", strings.export_done, path.display())`.
- `core/alert.rs`: `format!("القرص شارف على الامتلاء: {}", disk.mount_point)` — وهذا يظهر في إشعار سطح المكتب.

**الحل:** `sysmedic_core::lang::isolate(&path.display().to_string())` في الأربعة.

**(د) علامة النسبة المئوية تخالف السياسة الموثّقة.** رأس `knowledge.yaml`: *"Percentages in prose use the Arabic percent sign ٪ (U+066A), never `%`"*. لكن `core/alert.rs`:

```rust
Lang::Ar => format!("استُخدم {used:.0}% من {}", disk.mount_point),
// ...
Lang::Ar => format!("لم يتبقَّ سوى {avail:.0}% من الذاكرة"),
```

فالإشعار يقول "92%" والنتيجة في نفس اللحظة تقول "92٪". والاختبار يثبّت `%`. **الحل:** ٪ في الموضعين وعدّل الاختبار.

**(هـ) الجمع العربي مُهمَل في كل مكان.** `"{} {}"` مع `findings_count: "نتيجة"` ⇒ "5 نتيجة"؛ `alerts: "تنبيه."` ⇒ "3 تنبيه."؛ `disk_entries: "عنصر"` ⇒ "184320 عنصر"؛ `fixes.rs` "سيزيل {} حزمة إجمالاً" ⇒ "3 حزمة". الإنجليزية عندها "(s)"؛ العربية تحتاج: 1 نتيجة / 2 نتيجتان / 3–10 نتائج / 11+ نتيجة. **الحل:** دالة نقية `lang::count_ar(n, "نتيجة", "نتيجتان", "نتائج")` في `core` مع اختبار، تُستعمل في المواضع الأربعة.

**(و) بقايا إنجليزية في `cli/main.rs`** بغضّ النظر عن `--lang`: "Running SysMedic checkup..."، "Report written to"، "PDF report written to"، "Deep explanation (…):"، "--deep needs a Claude API key". **الحل:** أضفها إلى `Tools`.

**(ز) لغة الـ helper لا تتبع `--lang`.** `cli/fix.rs` → `delegate` يستبدل العملية بـ `pkexec` بلا تمرير اللغة، والـ helper يقرأ `Lang::from_env()`. فـ `sysmedic fix fix.enable_ufw --yes --lang ar` يطبع "جارٍ طلب التفويض" ثم يرد الـ helper "Applied fix.enable_ufw (Enable the firewall)". pkexec يحفظ `LC_MESSAGES` ضمن بيئته الدنيا. **الحل:** `.env("LC_MESSAGES", match lang { Ar => "ar", En => "en" })` على أمر pkexec في `delegate` وفي الواجهة.

### 3.5 إمكانية الوصول — 7.5/10

الخريطة `AccessibleRole::Img` بتسمية، قابلة للتركيز والتنقل بالأسهم مع إعلان البلاطة، وقائمة نصية تحتها، وتقليل الحركة محترم. خصمان:
- **أزرار الأيقونات بلا اسم للوصول.** `gui/ui.rs`: `gtk::Button::from_icon_name("view-refresh-symbolic")` مع `set_tooltip_text` فقط؛ و`gui/disk.rs`: زرا `folder-open-symbolic` و`process-stop-symbolic` كذلك. GTK4 يستعمل التلميح كـ *description* لا كـ *name*، فقارئ الشاشة يقرأ "button". **الحل:** `button.update_property(&[gtk::accessible::Property::Label(strings.run_checkup)])` للثلاثة.
- القائمة النصية تحت الخريطة 8 صفوف مقابل 60 بلاطة (ملاحظة سابقة، ما زالت).
- `.desktop`: `Keywords` بلا `Keywords[ar]` فالبحث في GNOME Shell بالعربية لا يجد التطبيق.

### 3.6 التغذية الراجعة — 7.5/10

التوستات صحيحة، والفشل يُعرض بحوار مع كلام الـ helper. الخصم: الحوار يصف الفشل بعد المصادقة بأنه فشل مصادقة (2.4 أ)؛ والمعاينة في الواجهة مبنية وقت الفحص لا وقت النقر، فمن يترك النافذة مفتوحة ساعات ثم يضغط "تطبيق" يقرأ أرقاماً قديمة ("يحرّر نحو 2.1 غيغابايت") — وهذا جزء من 4.2(ج).

### **متوسط قسم UX/UI: 7.8 / 10**

(8.5 + 7.5 + 9 + 7 + 7.5 + 7.5) ÷ 6 = 7.83

---

## 4. المراجعة الأمنية (25%)

**تحديد النطاق أولاً:** تطبيق سطح مكتب محلي بلا خادم، بلا مصادقة مستخدمين، بلا قاعدة بيانات، وبلا مدخلات شبكة (عدا استجابة API اختيارية). لذلك بنود "CORS / الترويسات / JWT / تجزئة كلمات المرور / حقن SQL" **لا تنطبق** ولم تُحتسب. سطح الهجوم الحقيقي: (1) الأداة المميّزة وحدّها، (2) ما تفعله الإصلاحات بالنظام وصدق عقد الموافقة، (3) البيانات المحلية المكتوبة، (4) بيانات النظام غير الموثوقة المعروضة، (5) الخروج الاختياري للشبكة، (6) سلسلة التوريد.

### 4.1 ما هو مُحكَم فعلاً (لا يحتاج عملاً)

- **نموذج الصلاحيات:** لا root في GUI/CLI؛ إجراءان polkit بـ `auth_admin`؛ helper يتحقق `is_root()`؛ يقبل معرّفاً فقط ويرفض معرّفات الطبقة الأخرى (اختبار بمعرّفات عدائية)؛ `SYSMEDIC_HELPER` مُصرَّف خارج release؛ الـ helper يجمع فقط ما يعلنه الإصلاح ويرفض بالاختبار أي إصلاح يطلب كل المجمّعات.
- **لا صدفة (shell)** في أي مسار؛ `PATH` معقّم؛ `DEBIAN_FRONTEND=noninteractive` + `stdin` مغلق + مهلة 10 دقائق؛ قيمتا `restore` (retain وقاعدة ufw) تُتحقّقان بصرامة قبل أن تلمسهما عملية root (اختبارات بمدخلات عدائية).
- **الـ journal:** سلامة لا سرّية (`0644`/`0755` مملوكة لـ root)، كتابة atomic عبر temp + `rename` + `O_NOFOLLOW`، `undo` يتجاهل الأوامر المخزّنة فيه.
- **الملفات الحساسة `0600`** (تقارير، تاريخ)، لا احتياط إلى `/tmp`، PDF عبر `tempfile` خاص وبلا `--no-sandbox`.
- **تهريب المخرجات:** `esc()` لـ HTML، `md_inline()` لـ Markdown، `sanitize()` للطرفية، `stderr_tail()` للحوار، `markup_escape_text` في GTK — كلها باختبارات حقن.
- **الاعتماديات:** `rustsec/audit-check` بوابة، Dependabot أسبوعي، Actions على SHA، `permissions: contents: read`، `--locked`، MSRV محروس. **لا أسرار في المستودع.**
- **LLM:** تحقّقت من مرجع API الحالي: `claude-opus-5` معرّف صالح ومُقدَّم، و`output_config.effort` بالشكل الصحيح (GA، داخل `output_config`)، و`MAX_TOKENS 16_000` مناسب لنموذج يفكّر قبل الإجابة. المفتاح من البيئة فقط ولا يُطبع، مهلتا اتصال وقراءة، والرفض الأمني (`stop_reason: refusal`) يصل كمحتوى فارغ فيعامَل كـ "empty response" ويُرجع للجواب الأوفلاين — تدهور صحيح.

### 4.2 الملاحظات

**🔴 حرج — `fix.enable_ufw` يقفل المستخدم البعيد إذا كان SSH على منفذ غير 22 (`fixes/fixes.rs`).**

الجولة السابقة أضافت استثناء SSH قبل التفعيل. التنفيذ في `EnableUfw::ssh_reachable`:

```rust
const SSH_PORT: u16 = 22;
const SSH_ALLOW_RULE: &str = "22/tcp";

fn ssh_reachable(s: &Snapshot) -> bool {
    let listening = s.ports.as_ref().is_some_and(|ports| {
        ports.iter().any(|p| p.exposed && p.port == SSH_PORT && p.proto.starts_with("tcp"))
    });
    let configured = s.security.as_ref().is_some_and(|sec| {
        sec.ssh_permit_root_login.is_some() || sec.ssh_password_auth.is_some()
    });
    listening || configured
}
```

نقل SSH عن المنفذ 22 (`Port 2222` في `sshd_config`) من أكثر ممارسات التحصين شيوعاً. على جهاز كهذا:
- `listening` = false (المنفذ 2222 لا يساوي 22).
- إذا كان `configured` = true (أي `sshd_config` أو `sshd_config.d/*.conf` فيه `PasswordAuthentication` صريحة — وهو حال كل صورة سحابية بـ cloud-init): الخطة تضيف `ufw allow 22/tcp` (منفذ لا يستمع عليه أحد) ثم `ufw --force enable` بسياسة `deny incoming` ⇒ **2222 محجوب، الجلسة التالية مرفوضة، والتراجع يحتاج الوصول الذي ضاع**.
- إذا كان `configured` = false: لا استثناء أصلاً ⇒ نفس القفل.

والأسوأ أن المعاينة تطمئن المستخدم صراحةً: *"This machine answers on SSH, so port 22/tcp is allowed first — without that, enabling the firewall would lock out every new remote login."* ثم تحتها تقول "Reachable from the network today and blocked afterwards: 2222/tcp" — الجملتان معاً هما التناقض الذي يُفترض أن يمنعه الإصلاح السابق. `CONTRIBUTING.md` ينصّ: *"A plan must not be able to leave the machine less reachable than it found it"* — والخطة هنا تخالفه.

**الحل:** في `collectors/security.rs` أضف `parse_ssh_ports(config) -> Vec<u16>` تقرأ كل توجيه `Port` قبل أول `Match` (sshd يقبل أكثر من `Port`، والافتراضي 22 عند غيابه) إلى حقل جديد `SecurityInfo::ssh_ports: Vec<u16>`. في `ssh_reachable` اعتبر أي منفذ TCP مكشوف ضمن `ssh_ports` منفذ SSH، وأضف `ufw allow <port>/tcp` لكل واحد، وخزّن القواعد في `restore` مفصولة بفاصلة (ووسّع `parse_allow_rule` ليقبل قائمة مع نفس التحقق لكل عنصر). اختبار إلزامي: لقطة بـ `Port 2222` + منفذ 2222 مكشوف ⇒ الأمر الأول `ufw allow 2222/tcp` ولا ذكر لـ 22.

**🟠 متوسط — ما وافق عليه المستخدم ليس بالضرورة ما يُنفَّذ (`gui/ui.rs`, `cli/fix.rs`, `fix-helper/src/lib.rs`).**

تفصيله الهندسي في 2.1. أمنياً هذا عقد الموافقة (consent contract) نفسه. ثلاثة أمثلة واقعية حيث تختلف الخطة المعروضة عن المنفَّذة:
1. **`fix.enable_ufw`:** `sshd_config.d/*.conf` قد تكون `0600` (تكتبها أدوات إدارة بصلاحيات مشدّدة)؛ غير المميّز لا يقرؤها ⇒ المعاينة بلا استثناء SSH، والـ helper كـ root يقرؤها ⇒ يضيف `ufw allow 22/tcp` لم يرَه المستخدم. (الاتجاه هنا آمن، لكن المبدأ مكسور.)
2. **`fix.journal_vacuum`:** `journalctl --disk-usage` لمستخدم خارج مجموعة `adm`/`systemd-journal` يحسب سجلاته هو فقط؛ كـ root يحسب كل شيء. فالخطة المعروضة "1.2 GiB" والمنفَّذة تقلّص 9 GiB.
3. **كل الإصلاحات:** الخطة في الواجهة مبنية وقت الفحص؛ النقر قد يأتي بعد ساعات والنظام تغيّر.

**الحل (لا يوسّع سطح الحقن):** بصمة الخطة. في `core/fix.rs` أضف `FixPlan::fingerprint() -> String` = SHA-256 لـ `commands` (برنامج + وسائط مرتّبة) — بلا اعتمادية جديدة إن استعملت تنفيذاً بسيطاً، أو `sha2` (مدقَّقة ومستقرة). الواجهة والـ CLI يمرّران `apply <id> <fingerprint>`؛ الـ helper يتحقق أن الوسيط 64 حرفاً سداسياً **قبل** أي عمل، يعيد بناء الخطة كالمعتاد، ويرفض بـ *"the system changed since the preview; run the checkup again"* إذا اختلفت البصمة. الإدخال الجديد ثابت الطول ومُتحقَّق بالشكل، والـ helper ما زال لا يأخذ أمراً ولا خطة من الخارج.

**🟠 متوسط — حوار الموافقة على `fix.flatpak_unused` يعرض عدداً وقائمة خاطئين، والقاعدة تطلق على كل سطح مكتب Flatpak (`collectors/flatpak.rs`).**

```rust
let needed: Vec<&str> = app_runtimes.lines()...;   // عمود runtime لكل تطبيق
runtimes.lines().filter_map(|line| {
    // ... runtime "needed" only if some app's runtime column equals it exactly
    (!is_needed).then(|| reference.to_string())
})
```

`flatpak list --runtime` يُدرج **الامتدادات** أيضاً: `org.gnome.Platform.Locale/x86_64/47`، `org.freedesktop.Platform.GL.default/x86_64/24.08`، `…GL.default/x86_64/24.08extra`، `…Platform.openh264/x86_64/2.4.1`، `org.gtk.Gtk3theme.*`، وامتدادات اللغة للتطبيقات نفسها (`org.mozilla.firefox.Locale`). لا تطبيق يذكر أياً منها في عمود `runtime`، فالمقارنة الحرفية تصنّفها كلها "غير مستخدمة". النتيجة على أي جهاز فيه تطبيق Flatpak واحد: `flatpak.unused_runtimes` تطلق بعدد 4–10، و"تطبيق الإصلاح" يظهر، وحوار الموافقة يقول *"Uninstall 7 Flatpak runtime(s) that no installed app needs"* — بينما `flatpak uninstall --unused` (الذي يحسب التبعيات صحيحاً) لن يزيل شيئاً أو يزيل أقل بكثير. مدخلة journal "Applied" لعملية لم تفعل شيئاً. الفيكستشر في الاختبار لا يحوي امتداداً واحداً ولذلك لم يُكتشف.

**الحل:** في `unused_refs` اعتبر المرجع مستخدَماً إذا كان معرّفه يبدأ بـ `<id>.` لأي runtime مستخدَم أو لأي تطبيق مثبّت (مرّر عمود `application` للتطبيقات أيضاً) — الامتدادات تحمل اسم أصلها بادئةً. أضف فيكستشر بـ `.Locale` و`.GL.default` و`.openh264` ويجب أن يرجع فارغاً. وبما أن الـ helper يعمل كـ root، فـ `flatpak uninstall --unused` يلمس التثبيت النظامي فقط لا `--user` للمستخدم؛ أضف `--system` صراحةً للأمر وللوصف حتى لا يَعد الحوار بما لا يُنفَّذ.

**🟠 متوسط — نتيجة Critical "ارتفاع حرارة" كاذبة من مستشعرات GPU/NVMe تقلب الدرجة إلى "ضعيفة" وتُرسل إشعاراً حرجاً يومياً (`collectors/thermal.rs`, `core/thresholds.rs`).**

`read_hwmon` يجمع كل `temp*_input` من كل شريحة بلا تمييز، و`thermal::overheating` و`alert::evaluate_in` يطبّقان عتبة واحدة (85/95°م) على **الأسخن** منها. لكن `amdgpu` يعرض `junction` (hotspot) الذي يعمل طبيعياً عند 95–105°م وحدّه الحرج 110؛ وNVMe يعرض مستشعرات بحدّ 80–85؛ و`iwlwifi` مشابه. فحاسوب ألعاب أو محطة عمل تحت حمل عادي يحصل على `thermal.overheating` **Critical** ⇒ الدرجة الكلية تُقصّ عند 59 "ضعيفة" (CRITICAL_SCORE_CAP) ⇒ وإشعار سطح مكتب بأولوية `critical` مع كل تشغيل مجدول. أداة اسمها طبيب تصرخ "خطر" على جهاز سليم تفقد مصداقيتها في أول أسبوع. الـ kernel يوفّر الحدّ الصحيح لكل مستشعر: `temp{N}_crit` و`temp{N}_max` بجانب `temp{N}_input`، و`trip_point_*_temp` لمناطق `thermal_zone`.

**الحل:** اقرأ `_crit` (ثم `_max` احتياطاً) لكل مستشعر إلى `ThermalSensor::crit_c: Option<f64>`، وفي القاعدة والتنبيه استعمل `temp >= crit - 5` ⇒ Critical و`temp >= 0.9 * crit` ⇒ High عند توفره، وعتبات `thresholds::thermal` فقط عند غيابه. اختبار بمستشعر `junction` عند 100°م وحدّه 110 يجب ألا يطلق.

**🟡 بسيط — `services.failed` بدرجة High لأي وحدة، فوحدة ضجيج واحدة تقصّ الدرجة عند 74 "مقبولة" (`diagnostics/rules.rs` → `services::failed_units`).** `fwupd-refresh.service` و`snap.*.service` و`apt-news.service` تفشل على Ubuntu بشكل روتيني بلا أثر. مع HIGH_SCORE_CAP، جهاز ممتاز بوحدة واحدة منها يُعرض "مقبولة 74". **الحل:** Medium لوحدة واحدة أو اثنتين، High لثلاث فأكثر، مع قائمة وحدات معروفة بالضجيج تُخفَّض إلى Low.

**🟡 بسيط — الـ `.deb` بلا `prerm`/`postrm`** فحذف الحزمة يترك `/var/lib/sysmedic/journal.json` و`/usr/share/polkit-1/actions/…`. الأخير يُزال تلقائياً (ملف مُدار)، الأول لا. وثّقه أو أضف `postrm purge`.

### **درجة القسم الأمني: 8.0 / 10**

الأساس (الحدّ المميّز، التحقق، الملفات، سلسلة التوريد) في مستوى 9. الخصم من فئة "صدق الإصلاح": مسار قفل جديد في الإصلاح الذي يُفترض أنه الأكثر أماناً (🔴)، وحوار موافقة يمكن أن يعرض غير ما يُنفَّذ (🟠)، ونتيجتان كاذبتان بأثر على الدرجة والإشعارات (🟠 ×2).

---

## 5. الدرجة الإجمالية المرجّحة

| الناحية | الدرجة | الوزن | المساهمة |
|---|---|---|---|
| تقنية وهندسية | 8.5/10 | 45% | 3.83 |
| UX/UI | 7.8/10 | 30% | 2.34 |
| أمن سيبراني | 8.0/10 | 25% | 2.00 |
| **الإجمالي** | | | **8.2/10** |

الحساب: (8.5 × 0.45) + (7.8 × 0.30) + (8.0 × 0.25) = 3.825 + 2.34 + 2.0 = **8.17 ≈ 8.2**

**ما تعنيه الدرجة:** مشروع ناضج هندسياً فوق المتوسط بكثير، بثقافة اختبار وتوثيق نادرة في مشاريع بهذا الحجم. ما يمنعه من 9 ليس البنية بل **صدق ما يقوله للمستخدم**: إصلاح "آمن قابل للتراجع" يمكن أن يقفل خادماً، وحوارات موافقة وقواعد تشخيص تعرض أرقاماً أو تحذيرات لا تطابق الواقع، وواجهة عربية تَعِد بالكثير وتتعثر في الخريطة والتنبيهات والجمع. كلها قابلة للإصلاح في جولة واحدة.

---

## 6. خطة الإصلاح المرتبة بالأولوية

| # | الأولوية | الناحية | المشكلة | الحل المقترح | التأثير المتوقع بعد الحل |
|---|---|---|---|---|---|
| 1 | 🔴 حرج | أمن | `fix.enable_ufw` يستثني المنفذ 22 فقط؛ SSH على `Port 2222` يُحجب والمعاينة تدّعي الأمان (`fixes/fixes.rs` → `ssh_reachable`) | `parse_ssh_ports()` في `collectors/security.rs` → `SecurityInfo::ssh_ports`؛ استثناء لكل منفذ SSH مكشوف؛ `restore` قائمة قواعد مع تحقق لكل عنصر؛ اختبار بـ 2222 | لا قفل بعيد مهما كان منفذ SSH؛ المعاينة صادقة |
| 2 | 🟠 مهم | أمن | الخطة المعروضة والمنفَّذة تُبنيان من لقطتين مختلفتين (`ui.rs`، `cli/fix.rs`، `fix-helper/lib.rs`) | `FixPlan::fingerprint()` (SHA-256 للأوامر)؛ `apply <id> <fp>`؛ الـ helper يتحقق من الشكل ويرفض عند الاختلاف | المستخدم يوافق على ما يُنفَّذ حرفياً؛ لا توسيع لسطح الحقن |
| 3 | 🟠 مهم | أمن/UX | `flatpak.unused_runtimes` يعدّ الامتدادات (`.Locale`، `.GL.default`) غير مستخدمة؛ حوار الموافقة يعرض قائمة خاطئة (`collectors/flatpak.rs` → `unused_refs`) | مطابقة بالبادئة `<id>.` ضد runtimes المستخدمة والتطبيقات المثبّتة؛ فيكستشر بامتدادات؛ `--system` في الأمر والوصف | لا نتيجة كاذبة على كل سطح مكتب Flatpak؛ الحوار يطابق ما يُحذف |
| 4 | 🟠 مهم | أمن/UX | عتبة حرارة واحدة على أسخن مستشعر؛ `amdgpu/junction` وNVMe يطلقان Critical كاذباً يقصّ الدرجة عند 59 ويُرسل إشعاراً يومياً (`collectors/thermal.rs`) | اقرأ `temp*_crit`/`_max` إلى `ThermalSensor::crit_c`؛ القاعدة والتنبيه نسبيان عند توفره | لا "خطر" على جهاز سليم؛ مصداقية الدرجة |
| 5 | 🟠 مهم | UX | `fix.snap_retain` يبقى معروضاً للأبد بعد تطبيقه، وكل ضغطة = كلمة مرور + مدخلة journal بلا أثر (`fixes.rs` → `SnapRetain::plan`؛ `snap.rs` → `parse_disabled_revisions`) | `None` إذا `refresh_retain <= 2`؛ عدّ النسخ المعطّلة لكل snap وأطلق القاعدة عند ≥ 2 | يختفي الإصلاح بعد تطبيقه؛ تزول 5 نقاط خصم دائمة |
| 6 | 🟠 مهم | هندسة/UX | `failure_body` يعرض "أُلغيت المصادقة أو فشلت" لحالة `Failed` التي تعني أن المصادقة نجحت والأمر فشل (`viewmodel.rs`) | نص جديد "شُغِّل الإصلاح لكنه لم يكتمل. قالت أداة الإصلاح:" + تحديث الاختبار الذي يثبّت الخطأ | المستخدم يفهم أين فشل الإصلاح فعلاً |
| 7 | 🟠 مهم | RTL | أسماء المجلدات العربية في الخريطة الشجرية تُرسم بحروف مفكّكة معكوسة وبلا قصّ (`disk.rs` → `draw_treemap` → `cr.show_text`) | Pango layout (`create_pango_layout` + `set_width` + `EllipsizeMode::End` + `pangocairo::show_layout`) | `المستندات` و`التنزيلات` تُقرأ؛ لا تداخل نص |
| 8 | 🟡 تحسين | RTL | الشريط الزمني معكوس في العربية والتلميح يقول "الأحدث على اليمين" (`ui.rs` → `report_view`) | `format!("\u{2066}{spark}\u{2069}{trend}")` | المحور الزمني يطابق التلميح |
| 9 | 🟡 تحسين | RTL | مسارات غير معزولة bidi في `scanning_label`، نتيجة الفحص، توست التصدير، وتنبيهات `alert.rs`؛ و`%` بدل `٪` في التنبيهات رغم السياسة الموثّقة (الاختبار يثبّتها) | `lang::isolate()` في الأربعة؛ ٪ في `alert.rs` + تعديل `alert_text_is_localized` | مسارات تُقرأ في الواجهة والإشعارات؛ طباعة موحّدة |
| 10 | 🟡 تحسين | RTL | الجمع العربي مُهمَل: "5 نتيجة"، "3 تنبيه."، "184320 عنصر"، "3 حزمة" (`text.rs`، `tools.rs`، `disk.rs`، `fixes.rs`) | `lang::count_ar(n, مفرد, مثنى, جمع)` نقية مختبَرة في `core` | عربية سليمة في الأماكن الأربعة |
| 11 | 🟡 تحسين | UX | التراجع يأخذ موافقة أضعف من التطبيق: العنوان فقط بلا وقت ولا أوامر (`ui.rs` → `install_undo_action`؛ `cli/fix.rs` → `undo`) | اعرض `applied_at` محلياً + `fix.undo_from(entry.restore)` في الحوار والمعاينة | موافقة مستنيرة على عملية root ثانية |
| 12 | 🟡 تحسين | UX | زر "تطبيق الإصلاح" يظهر داخل Flatpak حيث لا pkexec (`ui.rs` → `report_view`) | `viewmodel::fixes_available()` تفحص `/.flatpak-info`؛ سطر "الإصلاحات متاحة في .deb" بدل الزر | لا كلمة مرور تُطلب لعملية مستحيلة |
| 13 | 🟡 تحسين | i18n | `--lang` لا يصل إلى الـ helper فيردّ بالإنجليزية في جلسة عربية (`cli/fix.rs` → `delegate`؛ `ui.rs`) | `.env("LC_MESSAGES", "ar"/"en")` على أمر pkexec (يحفظه pkexec) | رسالة الـ helper بلغة الأمر |
| 14 | 🟡 تحسين | i18n | بقايا إنجليزية في `cli/main.rs` ("Running SysMedic checkup...", "Report written to", "--deep needs…") | أضفها إلى `Tools` في `text.rs` | الطرفية عربية بالكامل تحت `--lang ar` |
| 15 | 🟡 تحسين | وصول | أزرار الأيقونات (تحديث، اختيار مجلد، إيقاف) بلا اسم للوصول؛ `.desktop` بلا `Keywords[ar]` | `update_property(&[Property::Label(...)])` للثلاثة؛ `Keywords[ar]=صحة;طبيب;تشخيص;تنظيف;مراقبة;نظام;` | قارئ الشاشة يسمّي الأزرار؛ البحث العربي في Shell يجد التطبيق |
| 16 | 🟡 تحسين | هندسة | رمز الخروج 1 = "High" و= "خطأ تشغيل" معاً (`cli/main.rs`) | `ExitCode::from(3)` للأخطاء ووثّقه في `--help` | سكربتات المراقبة تفرّق بين تدهور وانهيار |
| 17 | 🟡 تحسين | هندسة | journal تالف يعطّل كل الإصلاحات بلا مخرج (`fix-helper/lib.rs` → `open_journal`؛ `journal.rs` → `load`) | عند `Corrupt`: انقله إلى `journal.json.corrupt-<ts>` وابدأ فارغاً مع تحذير | تعافٍ تلقائي مع حفظ سجل التدقيق |
| 18 | 🟡 تحسين | DRY | استدعاء pkexec مكرّر في `confirm_and_apply` و`run_helper`؛ `FixOutcome` بقايا؛ `undo_commands()` بلا مستهلك (`ui.rs`، `fixes.rs`) | `run_helper(helper, Vec<String>)` من الموضعين؛ احذف `FixOutcome` و`undo_commands` | نسخة واحدة من المسار المميّز في الواجهة |
| 19 | 🟡 تحسين | هندسة | `services.failed` High لأي وحدة؛ `fwupd-refresh.service` وحدها تقصّ الدرجة عند 74 (`rules.rs` → `failed_units`) | Medium لـ 1–2، High لـ ≥ 3، قائمة وحدات ضجيج معروفة → Low | درجة تعكس الحالة لا الضجيج |
| 20 | 🟡 تحسين | توثيق | `ARCHITECTURE.md` (`claude-opus-4-8`، `sysmedicd` M5)؛ `INSTALL.md` `claude-sonnet-5`؛ `DEFAULT_MODEL` جيل أقدم وأغلى من `claude-opus-5-5`؛ تعليق `history.rs` بائت؛ لا وسم git فسير عمل Release لم يُشغَّل قط | صحّح النصوص الثلاثة؛ `DEFAULT_MODEL = "claude-opus-5-5"` + المثال `claude-sonnet-5-5`؛ ادفع وسم `v0.2.0` وتحقق من SHA256SUMS والـ attestation فعلياً | وثائق تطابق الكود؛ أول إصدار موقَّع فعلي |

**ترتيب التنفيذ المقترح:** 1 → 6 → 5 → 3 → 4 (كلها اختبار-أولاً) → 2 → 7 → 8–10 (دفعة RTL واحدة) → الباقي. البنود 1 و5 و6 و8 و9 أقل من ساعتين مجتمعة؛ 2 و7 نصف يوم لكل منهما؛ 3 و4 تحتاجان فيكستشرات من أجهزة حقيقية.
