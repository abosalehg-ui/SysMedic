# مراجعة هندسية شاملة — SysMedic

**تاريخ المراجعة:** 2026-08-03 · **الفرع:** `claude/senior-code-review-j725h8` · **آخر commit:** `8b019cf`
**نوع المراجعة:** قراءة فقط — لم يُعدَّل أي ملف في المستودع عدا هذا التقرير.

---

## الحكم النهائي

مشروع Rust مكوّن من 11 crate يقدّم أداة تشخيص وإصلاح لنظام لينكس (CLI + تطبيق GTK4). هذا **ليس مشروعاً شخصياً عادياً**: الفصل المعماري متعمَّد، وحدود الثقة في المسار المُمتاز (privileged) مصمَّمة بوعي أمني حقيقي، و127 اختباراً تمر، و`clippy -D warnings` و`rustfmt` نظيفان تماماً، وCI يثبّت الـ actions بـ commit SHA ويشغّل بوابة RUSTSEC. الدرجة المرجّحة **7.7/10** بملف الأوزان **A** (تطبيق بواجهة مستخدم: هندسة 45% / UX 30% / أمن 25%).

**هل المشروع جاهز للاستخدام/النشر؟ → بشروط.** جاهز عبر حزمة `.deb` فقط؛ الإصلاحات المميّزة معطّلة فعلياً في Flatpak وSnap وAppImage، والتطبيق يُشحن بلا أيقونة.

أهم ثلاث مشكلات:
1. `OpenOptions::mode(0o600)` لا يُطبَّق على ملف موجود مسبقاً — تصدير تقرير فوق ملف قديم يُبقيه مقروءاً للجميع (`sysmedic-cli/src/main.rs` و`sysmedic-gui/src/ui.rs`).
2. حوار الموافقة على إصلاح مميّز يعرض العنوان والوصف بالإنجليزية للمستخدم العربي، وتنبيهات سطح المكتب إنجليزية بالكامل بلا أي معامل لغة (`sysmedic-core/src/fix.rs` و`sysmedic-core/src/alert.rs`).
3. `apply()` ينفّذ الأوامر ثم يكتب السجل — فشل الكتابة يترك الإصلاح مُطبَّقاً وغير قابل للتراجع (`sysmedic-fixes/src/lib.rs`).

---

## 1. نظرة عامة

**ما يفعله المشروع:** يجمع لقطة (`Snapshot`) عن حالة النظام عبر 16 مجمّعاً، يمرّرها على 28 قاعدة تشخيصية نقية، يحسب درجة صحّة موزونة من 0 إلى 100 عبر 12 فئة، يشرح كل نتيجة بالعربية والإنجليزية من قاعدة معرفة مضمّنة تعمل دون إنترنت، ويعرض إصلاحات آمنة تُنفَّذ عبر مساعد مُصرَّح له من polkit.

**البنية:** طبقات نظيفة بترتيب تبعية أحادي الاتجاه:
`core` (نموذج المجال، لا I/O) ← `collectors` (I/O فقط) ← `diagnostics` (قواعد نقية) ← `knowledge` / `fixes` / `report` / `history` / `diskscan` ← `cli` / `gui` / `fix-helper`.

**المكدّس التقني** (من ملفات المانيفست الفعلية، لا من الذاكرة):

| العنصر | الإصدار | المصدر |
|---|---|---|
| Rust edition | 2021 | `Cargo.toml` |
| إصدار المشروع | 0.2.0 | `Cargo.toml` |
| الترخيص | GPL-3.0-or-later | `Cargo.toml` + `LICENSE` |
| gtk4 | 0.9.7 (ميزات `v4_14`) | `Cargo.lock` |
| libadwaita | 0.7.2 (ميزات `v1_5`) | `Cargo.lock` |
| clap | 4.6.4 | `Cargo.lock` |
| serde / serde_json | 1.x | `Cargo.toml` |
| **serde_yaml** | **0.9.34+deprecated** | `Cargo.lock` |
| ureq | 2.12.1 (rustls، بلا TLS نظامي) | `Cargo.lock` |
| rustls | 0.23.42 | `Cargo.lock` |
| thiserror | 2.0.19 | `Cargo.lock` |
| toolchain المستخدم في المراجعة | cargo 1.94.1 | `cargo --version` |

**الأرقام:**

| المقياس | القيمة |
|---|---|
| ملفات متتبَّعة في git | 96 |
| ملفات `.rs` | 49 |
| أسطر Rust (تشمل الاختبارات والتوثيق) | 9331 |
| عدد الـ crates | 11 |
| قواعد تشخيصية | 28 |
| إصلاحات آمنة | 6 |
| اختبارات ناجحة | 127 |
| المساهمون | ABDULKARIM ALOBUD (13 commit) · Claude (32 commit) |

**أكبر الملفات:** `sysmedic-diagnostics/src/rules.rs` (995 سطراً)، `sysmedic-gui/src/ui.rs` (561)، `sysmedic-report/src/lib.rs` (408).
**أكثر الملفات تغيّراً:** `Cargo.lock` (13)، `README.md` (12)، `sysmedic-gui/src/ui.rs` (11)، `.github/workflows/ci.yml` (10).

**التصنيف:** ملف الأوزان **A — تطبيق بواجهة مستخدم**. المشروع يشحن تطبيق GTK4 كاملاً بالإضافة إلى CLI، فوزن UX البالغ 30% مبرَّر؛ ملف B (مكتبة/CLI) كان سيُهمل الواجهة الرسومية التي تمثّل ثلث الشيفرة الفعلية، وملف D (خدمة خلفية) لا ينطبق إذ لا يوجد أي مكوّن خادم أو شبكة واردة.

---

## 2. المراجعة التقنية والهندسية

### 2.1 المعمارية وفصل الاهتمامات — 9/10

هذه أقوى نقطة في المشروع، وأمنحها 9 بناءً على ثلاثة أدلة محدّدة لا على انطباع عام.

**الدليل الأول — حدود الثقة في المكوّن المميّز.** المساعد لا يقبل من مستدعيه إلا **معرّف إصلاح**، ويتحقق منه قبل أي عمل بصلاحيات root:

```rust
// crates/sysmedic-fix-helper/src/main.rs — parse_action
match args {
    [cmd, fix_id] if cmd == "apply" => {
        if !sysmedic_fixes::FIX_IDS.contains(&fix_id.as_str()) {
            return Err(format!("unknown fix id '{fix_id}'"));
        }
        Ok(Action::Apply(fix_id.clone()))
    }
```

ثم يعيد بناء الخطة بنفسه من اللقطة التي يجمعها هو، فلا يستطيع مستدعٍ مخترَق تهريب أمر. والأهم أن `undo` يطبّق المبدأ نفسه — يتجاهل الأوامر المخزّنة في ملف السجل ويعيد بناءها من السجل المُصرَّف:

```rust
// crates/sysmedic-fixes/src/lib.rs — undo
let commands =
    fixes::undo_commands(&fix_id).ok_or_else(|| FixError::UnknownFix(fix_id.clone()))?;
```

وهناك اختبار عدائي يوثّق هذا صراحةً (`undo_ignores_tampered_commands_in_the_journal`) يزرع `rm -rf /` في السجل ويتحقق أن المنفَّذ هو `ufw disable`.

**الدليل الثاني — الثوابت العددية مركزية وموحّدة.** `sysmedic-core/src/thresholds.rs` هو المصدر الوحيد للحقيقة، تستهلكه التنبيهات والقواعد والإصلاحات معاً، والتوثيق يشرح لماذا:

```rust
/// At/above this, `logs.journal_large` is raised Medium — and the
/// `fix.journal_vacuum` fix becomes applicable (same constant, so the
/// finding and its fix can never disagree about when trimming makes sense).
pub const LARGE_BYTES: u64 = 1024 * 1024 * 1024;
```

**الدليل الثالث — مفاصل (seams) حقيقية تجعل كل شيء قابلاً للاختبار دون لمس النظام.** `CommandRunner` للأوامر، `HttpTransport` للشبكة، `Collector`/`Diagnostic` للقواعد. نتيجة ذلك أن قواعد التشخيص الـ 28 كلها دوال نقية `fn(&Snapshot) -> Vec<Finding>` تُختبر ضد لقطات تجريبية.

وهناك **جذر تركيب واحد** (`default_engine`) مع تعليق يشرح الدَّين الذي أزاله:

```rust
// crates/sysmedic-diagnostics/src/lib.rs
/// The single composition root — CLI, GUI and the fix helper previously each
/// rebuilt this by hand, which is exactly how their configurations drift.
```

**لا توجد ملاحظات جوهرية** في هذا المحور.

### 2.2 قابلية القراءة والصيانة — 8/10

التعليقات تشرح **لماذا** لا **ماذا**، وهذا نادر. مثال يوثّق قراراً أمنياً مع سببه:

```rust
// data/io.github.abosalehg_ui.sysmedic.policy
<!-- auth_admin (not _keep): a cached authorization would let any process
     in the session re-invoke the helper (e.g. `undo` right after the user
     enabled the firewall) without a prompt. -->
```

**الخصم:** `crates/sysmedic-gui/src/ui.rs` بلغ 561 سطراً وتراكم فيه بناء الودجات وإجراءات التطبيق ومنطق الإصلاح والتصدير في دالة واحدة `build_window` تتجاوز 200 سطر. مقارنةً بانضباط بقية المستودع، هذا هو الملف الذي سيصعب تعديله لاحقاً — وهو بالمناسبة الأعلى churn بين ملفات المصدر (11 تعديلاً).

**الإصلاح المقترح:** استخراج تسجيل الإجراءات إلى دالة مستقلة.

```rust
// قديم — داخل build_window
let export_action = gio::SimpleAction::new("export", None);
export_action.connect_activate({ /* ~35 سطراً */ });
app.add_action(&export_action);
app.set_accels_for_action("app.export", &["<Ctrl>e"]);
```

```rust
// جديد
fn install_export_action(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    toasts: &adw::ToastOverlay,
    latest_report: Rc<RefCell<Option<HealthReport>>>,
    lang: Lang,
) { /* نفس المحتوى، خارج build_window */ }

// في build_window
install_export_action(app, &window, &toasts, latest_report.clone(), lang);
```

### 2.3 التكرار و code smells — 7/10 · `مؤكد`

خمسة مواضع تكرار حقيقي، كلها متطابقة نصياً بين ملفين أو أكثر:

**(أ) مسار المساعد المميّز مكرّر حرفياً في CLI وGUI.**

```rust
// crates/sysmedic-cli/src/fix.rs
const DEFAULT_HELPER: &str = "/usr/libexec/sysmedic-fix-helper";
fn helper_path() -> String {
    std::env::var("SYSMEDIC_HELPER").unwrap_or_else(|_| DEFAULT_HELPER.to_string())
}
```

```rust
// crates/sysmedic-gui/src/ui.rs — نفس الثابت ونفس الدالة حرفياً
const DEFAULT_HELPER: &str = "/usr/libexec/sysmedic-fix-helper";
fn helper_path() -> String {
    std::env::var("SYSMEDIC_HELPER").unwrap_or_else(|_| DEFAULT_HELPER.to_string())
}
```

هذا مسار أمني حسّاس — تكراره يعني أن تشديده في مكان واحد يترك الآخر مكشوفاً.

**(ب) `SAFE_PATH` مكرّر** بنفس القيمة في `sysmedic-collectors/src/util.rs` و`sysmedic-fixes/src/command.rs` (`"/usr/sbin:/usr/bin:/sbin:/bin"`).
**(ج) `write_private`** مكرّرة في `sysmedic-cli/src/main.rs` و`sysmedic-gui/src/ui.rs` بنفس المنطق.
**(د) استنتاج مسار XDG** مكرّر بين `sysmedic-fixes::journal_path` و`sysmedic-history::default_path` (نفس سلسلة `XDG_STATE_HOME` → `HOME/.local/state` → `/tmp`).
**(هـ) `FIX_IDS`** قائمة يدوية تكرّر ما تنتجه `all()` — يحميها اختبار `registry_and_ids_agree`، وهو تخفيف مقبول لكنه يبقى مصدرين للحقيقة.

**الإصلاح المقترح (أ):** نقل الثابت والدالة إلى `sysmedic-fixes`، الذي يعتمد عليه الطرفان أصلاً.

```rust
// جديد — crates/sysmedic-fixes/src/lib.rs
pub const DEFAULT_HELPER: &str = "/usr/libexec/sysmedic-fix-helper";
pub fn helper_path() -> String {
    std::env::var("SYSMEDIC_HELPER").unwrap_or_else(|_| DEFAULT_HELPER.to_string())
}
```

```rust
// جديد — في cli/src/fix.rs و gui/src/ui.rs معاً
use sysmedic_fixes::helper_path;
```

### 2.4 معالجة الأخطاء والحالات الحدّية — 7/10

الأساس قوي: `collection_errors` بدل إفشال الفحص كله، أخطاء مصنّفة عبر `thiserror`، ومجمّع يُصاب بـ panic يتحوّل إلى «فحص متخطّى» بدل تسميم النتيجة:

```rust
// crates/sysmedic-core/src/engine.rs — run
Err(name) => snapshot
    .collection_errors
    .push(format!("{name}: collector panicked")),
```

لكن ثلاث مشكلات محدّدة:

**(أ) ترتيب التنفيذ ثم التسجيل — `مؤكد` · تأثير: إصلاح غير قابل للتراجع.**

```rust
// crates/sysmedic-fixes/src/lib.rs — apply
for command in &plan.commands {
    outputs.push(runner.run(command).map_err(FixError::CommandFailed)?);
}
journal.record(JournalEntry { /* ... */ })?;   // ← إن فشلت، الإصلاح مُطبَّق بلا سجل
```

سيناريو الفشل: `/var/lib` ممتلئ (وهو سيناريو محتمل جداً في أداة يُشغّلها المستخدم **لأن قرصه ممتلئ**). `ufw --force enable` ينجح، ثم `journal.record` يفشل بـ `ENOSPC`، فيرى المستخدم رسالة خطأ ويظن أن شيئاً لم يحدث — بينما الجدار الناري صار مفعّلاً و`sysmedic undo` لن يجده أبداً.

```rust
// جديد — سجّل النية أولاً، ثم أكّدها
let index = journal.record_pending(&plan)?;   // يُكتب قبل تنفيذ أي أمر
let mut outputs = Vec::new();
for command in &plan.commands {
    outputs.push(runner.run(command).map_err(FixError::CommandFailed)?);
}
journal.confirm(index)?;
```

بديل أخفّ إن كان تغيير بنية السجل مكلفاً: التقاط خطأ `record` وإعادته مغلَّفاً برسالة تقول صراحةً إن الإصلاح **نُفِّذ** لكن تعذّر تسجيله.

**(ب) `mode(0o600)` يُتجاهَل على ملف موجود — `مؤكد`.** يُعالَج في القسم الأمني (4.5) لتفادي التكرار.

**(ج) ابتلاع صامت لأخطاء الصلاحيات.**

```rust
// crates/sysmedic-fixes/src/journal.rs — save
let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
```

التعليق يقول «best-effort on exotic filesystems»، لكن الحالة الواقعية الأخطر ليست نظام ملفات غريب بل **دليل يملكه مستخدم آخر** (سيناريو الرجوع إلى `/tmp`، انظر 4.5) — وحينها يفشل الاستدعاء بصمت ويستمر البرنامج بالكتابة في دليل غير محمي.

### 2.5 الأداء واستهلاك الموارد — 8/10

قرارات جيدة وموثّقة: المجمّعات تعمل على خيوط متزامنة (`std::thread::scope`) بدل التسلسل، مع تعليق يشرح المكسب؛ مهلة صارمة 10 ثوانٍ لكل أمر خارجي؛ خيط منفصل لتفريغ الأنبوب يمنع deadlock على مخزن ممتلئ؛ ومسح القرص يلتزم بحدود نظام الملفات مثل `du -x`:

```rust
// crates/sysmedic-diskscan/src/lib.rs — scan
// The walk stays on it, like `du -x`, so a scan of `/` doesn't wander into
// `/proc` (where `/proc/kcore` alone reports ~128 TiB), `/sys`, `/dev`.
let root_dev = fs::metadata(root).map(|m| m.dev()).ok();
```

**الخصم — `find()` تُخصّص السجل كاملاً في كل استدعاء · `مؤكد`:**

```rust
// crates/sysmedic-fixes/src/fixes.rs
pub fn find(id: &str) -> Option<Box<dyn Fix>> {
    all().into_iter().find(|f| f.id() == id)
}
```

`all()` تبني 6 `Box<dyn Fix>` جديدة في كل نداء، و`undo_commands` تستدعي `find` التي تستدعي `all`. الأثر تافه بالأرقام المطلقة (6 تخصيصات) لكنه نمط يتكرّر في مسار الواجهة: `report_view` تستدعي `fix_for_finding` ثم `plan` لكل نتيجة معروضة.

**الخصم الأهم — مسح القرص بلا إلغاء ولا تقدّم · `مؤكد`:** صفحة «استخدام القرص» تمسح `$HOME` كاملاً على خيط عامل، و`scan_inner` تنزل في الشجرة **بالكامل** بغضّ النظر عن `depth_left` (النزول ضروري لصحة المجاميع، والحدّ يقتصر على ما يُحتفظ به من أبناء). على مجلد منزل بمئات آلاف الملفات هذا يستغرق دقائق دون أي شريط تقدّم أو زر إلغاء، والرسالة تبقى «جارٍ فحص مجلد المنزل…» بلا تغيير.

### 2.6 الاختبارات والتغطية — 8/10

**شُغّلت الحزمة فعلياً.** الأمر ونتيجته الحقيقية:

```
$ cargo test --workspace --exclude sysmedic-gui
```

| الوحدة | ناجح | فاشل |
|---|---|---|
| sysmedic-cli (bin) | 9 | 0 |
| sysmedic-collectors | 53 | 0 |
| sysmedic-core | 17 | 0 |
| sysmedic-diagnostics | 5 | 0 |
| sysmedic-diskscan | 4 | 0 |
| sysmedic-fix-helper | 14 | 0 |
| sysmedic-fixes | 7 | 0 |
| sysmedic-history | 12 | 0 |
| sysmedic-knowledge | 6 | 0 |
| **الإجمالي** | **127** | **0** |

وكذلك:

```
$ cargo clippy --workspace --exclude sysmedic-gui --all-targets
    Finished `dev` profile ... (0 تحذيرات)
$ cargo fmt --all --check
    (خرج بصفر، لا فروقات)
```

جودة الاختبارات فوق المعتاد بوضوح — ليست اختبارات «تغطية شكلية». ثلاثة أمثلة:

- **اختبار شمولية معكوس:** `every_emitted_id_is_declared` يبني لقطة «مريضة قصوى» تُفعّل كل قاعدة، ثم يتحقق من الاتجاهين — أن كل معرّف منبعث مُعلَن، **وأن كل معرّف مُعلَن انبعث فعلاً**.
- **اختبار يمنع فجوة ترجمة:** `every_finding_has_arabic_title_and_summary_templates` يمرّ على `FINDING_IDS` ويرفض أي قالب عربي يشير إلى وسيط غير موجود:

```rust
let rendered = super::render_template(template, &probe);
assert!(!rendered.contains('{'),
    "template for {id} references an out-of-range arg: {template}");
```

- **اختبار عدائي** ضد تلاعب بملف السجل (ذُكر في 2.1).

**الخصم:** التغطية غير متجانسة. `sysmedic-report` و`sysmedic-diskscan` مُختبران جيداً، لكن **`sysmedic-gui` غير مُختبر عملياً**: الاختبارات الموجودة تغطي `viewmodel` و`disk::color_for` فقط، ولا شيء يغطي `build_window` أو `confirm_and_apply` — وهما بالضبط حيث يُتخذ قرار تشغيل أمر مميّز. لا توجد قياسات تغطية (`tarpaulin`/`llvm-cov`) في CI، ولا اختبارات تكامل على الملف التنفيذي عدا الـ smoke runs في CI.

### 2.7 التوثيق — 8/10

التوثيق حقيقي وصادق، لا تسويقي. `packaging/README.md` يعترف صراحةً بأخطر قيد في المشروع بدل إخفائه:

> Under **Flatpak, Snap, and AppImage** the polkit action's `exec.path` points at the host path `/usr/libexec/sysmedic-fix-helper`, which those sandboxed/bundled formats do not install on the host — so **Auto-Fix currently requires the `.deb`**.

و`SECURITY.md` يوثّق نموذج الثقة بدقة تطابق الشيفرة المقروءة.

**الخصم — أرقام معلَنة لا تطابق الشيفرة · `مؤكد`.** تحققتُ عدّياً:

```
$ sed -n '/pub const FINDING_IDS/,/^];/p' crates/sysmedic-diagnostics/src/lib.rs | grep -c '^    "'
28
$ grep -c 'rule!(' crates/sysmedic-diagnostics/src/lib.rs
28
$ grep -c '^- id:' crates/sysmedic-knowledge/data/knowledge.yaml
28
```

بينما التوثيق يقول:

| الملف | المكتوب | الصحيح |
|---|---|---|
| `README.md` | `27 rules` | 28 |
| `docs/site/index.html` | `27 rules` | 28 |
| `docs/site/ar.html` | `27 قاعدة` | 28 |
| `docs/ISSUES.md` | `21 diagnostic` | 28 |
| `docs/ROADMAP.md` | `21 diagnostic` | 28 |

الأخيران متقادمان بفارق 7 قواعد. ملاحظة إضافية: `INSTALL.md` يوجّه المستخدم إلى `packaging/` لصيغ Flatpak/Snap/AppImage **دون تحذيره** أن الإصلاحات لا تعمل فيها — التحذير موجود في `packaging/README.md` وحده.

**الإصلاح المقترح:** اشتقاق العدد بدل كتابته يدوياً غير ممكن في Markdown، فالبديل هو اختبار يحرس الرقم:

```rust
// جديد — crates/sysmedic-diagnostics/src/lib.rs
#[test]
fn declared_rule_count_matches_the_docs() {
    // إن تغيّر هذا الرقم، حدِّث README.md و docs/site/*.html
    assert_eq!(FINDING_IDS.len(), 28);
}
```

### 2.8 التسمية والاتساق الأسلوبي — 8/10

`cargo fmt --check` و`clippy -D warnings` نظيفان (تُنفَّذان في CI أيضاً). التسمية متسقة: معرّفات النتائج `category.snake_case`، معرّفات الإصلاحات `fix.snake_case`، أسماء القواعد `kebab-case`.

**الخصم — بادئة المعرّف تخالف الفئة في موضعين · `مؤكد`:**

```rust
// crates/sysmedic-diagnostics/src/rules.rs — packages::security_updates
Finding::new(
    "packages.security_updates",
    Category::Security,      // ← البادئة packages، الفئة Security
```

```rust
// crates/sysmedic-diagnostics/src/rules.rs — snap::old_revisions
Finding::new(
    "snap.old_revisions",
    Category::Storage,       // ← البادئة snap، الفئة Storage
```

في بقية الـ 26 قاعدة البادئة تطابق الفئة. هذا يكسر التوقّع الضمني الذي يبنيه المستخدم من `sysmedic checks`، ويُصعّب تجميع النتائج برمجياً حسب الفئة انطلاقاً من المعرّف.

### متوسط الفئة التقنية والهندسية

(9 + 8 + 7 + 7 + 8 + 8 + 8 + 8) ÷ 8 = **7.9/10**

---

## 3. المظهر والتصميم وتجربة المستخدم

### 3.1 الاتساق البصري — 8/10

التطبيق يستخدم أصناف Adwaita القياسية بدل ألوان مكتوبة يدوياً (`"success"` / `"warning"` / `"error"` / `"dim-label"`)، فيرث الثيم الداكن/الفاتح تلقائياً. ولوحة ألوان الخريطة الشجرية مُنتقاة من لوحة GNOME الرسمية مع تعليق يشرح قرار الاستبدال:

```rust
// crates/sysmedic-gui/src/disk.rs
/// A small curated palette (GNOME palette shades) instead of arbitrary
/// hash-derived HSL: the old saturated candy colors clashed with the
/// otherwise restrained Adwaita look.
const PALETTE: [(f64, f64, f64); 8] = [
    (0.11, 0.44, 0.85), // blue4    #1c71d8
```

وأشرطة الفئات تستخدم `LevelBar` مع عتبات لونية فعلية بدل شريط أحادي اللون:

```rust
bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_LOW, 50.0);
bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_HIGH, 75.0);
```

مع تعليق دقيق: «without them 76 and 100 render identically».

### 3.2 سهولة الاستخدام ووضوح التنقّل — 7/10

بنية واضحة: صفحتان (`overview` / `disk`) عبر `ViewStack` مع `ViewSwitcher` في الهيدر، اختصارات لوحة مفاتيح مسجّلة (`F5`، `Ctrl+R` للفحص، `Ctrl+E` للتصدير)، وقائمة رئيسية بها التصدير و«عن التطبيق».

**الخصم — `مؤكد`:** صفحة القرص تفحص `$HOME` **بشكل ثابت مبرمَج** بلا أي وسيلة لاختيار مجلد آخر:

```rust
// crates/sysmedic-gui/src/disk.rs — disk_page
let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
```

بينما نظيرتها في الطرفية تقبل مساراً (`sysmedic disk ~`). كذلك مربّعات الخريطة الشجرية غير قابلة للنقر — لا تعمّق (drill-down) ولا كشف للمسار الكامل، وهو التفاعل الأساسي المتوقَّع من أي محلّل قرص.

ملاحظة على السقوط إلى `"/"` عند غياب `HOME`: فحص الجذر كاملاً في خيط عامل بلا إلغاء (انظر 2.5) يجعل هذا المسار الاحتياطي أسوأ من عدم الفحص.

### 3.3 الاستجابة عبر نقاط التوقّف — 8/10

معالجة صريحة ومدروسة لا افتراضية:

```rust
// crates/sysmedic-gui/src/ui.rs — build_window
let breakpoint =
    adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 550sp").unwrap());
breakpoint.add_setter(&switcher_bar, "reveal", Some(&true.to_value()));
breakpoint.add_setter(&header, "title-widget", Some(&None::<gtk::Widget>.to_value()));
```

مع تعليق يشرح السبب: «the wide in-header switcher truncates instead of adapting on small windows». والنافذة تعلن حداً أدنى واقعياً (`width_request(360)`) يسمح بالعمل على شاشات ضيقة. صفحة الويب تستخدم `repeat(auto-fit, minmax(...))` و`@media (max-width: 560px)` لتقليل الأعمدة.

**ملاحظة صغيرة:** `.unwrap()` على `BreakpointCondition::parse` لسلسلة ثابتة مقبول (فشلها خطأ برمجي وقت التطوير لا وقت التشغيل).

### 3.4 دعم العربية وRTL — 6/10

**ما هو صحيح فعلاً** (وهو أكثر مما تفعله معظم المشاريع):

- `docs/site/ar.html` يعلن `<html lang="ar" dir="rtl">` ويستخدم خصائص CSS منطقية: `.lang-switch { inset-inline-end: 20px; }`.
- مكدّس خطوط عربي حقيقي مع بدائل: `"Noto Kufi Arabic", "Noto Sans Arabic"`.
- كتل الأوامر تُجبَر على LTR لمنع إعادة الترتيب ثنائي الاتجاه: `pre { direction: ltr; text-align: left; }`.
- الأرقام غربية (0-9) في كل النصوص العربية — اتساق تام.
- تقرير HTML يضبط الاتجاه واللغة معاً حسب اللغة المطلوبة، مع اختبار يحرسه (`html_is_rtl_aware_and_localizes_chrome`).
- ترجمة العربية ليست سطحية: الفئات والتقديرات ودرجات الخطورة وقوالب عناوين النتائج وملخصاتها كلها مترجمة، ويحرسها اختبار شمولية.

**الخصم الأول — نصّ الموافقة على إصلاح مميّز إنجليزي · `مؤكد` · وهو الأخطر في هذا المحور.** التوثيق نفسه يعترف:

```rust
// crates/sysmedic-core/src/fix.rs — preview_in
/// (The plan title and description come from the fix registry and are
/// English for now; the labels around the consent-critical facts — risk,
/// reversibility, exact commands — follow the user's language.)
```

والنتيجة أن المستخدم العربي يرى في حوار التأكيد:

```
Remove old kernels and orphaned packages
Purge 3 old kernel image(s) and any auto-installed packages no longer needed.

الخطورة: متوسطة
قابل للتراجع: لا
```

أي أن **التسميات مترجمة والمحتوى الذي يجب أن يُقرأ قبل الموافقة على تغيير غير قابل للتراجع ليس مترجماً**. هذا يعكس الأولوية الصحيحة رأساً على عقب.

**الخصم الثاني — تنبيهات سطح المكتب إنجليزية بالكامل · `مؤكد`.** `sysmedic-core/src/alert.rs` لا يستقبل معامل `Lang` إطلاقاً:

```rust
title: "System overheating".to_string(),
body: format!("{} at {:.0}°C", hottest.name, hottest.temp_c),
```

هذا يمسّ ركن «المتابعة» بالذات — الركن الذي يروّج له الموقع العربي بوصفه أحد الخطوات الخمس.

**الخصم الثالث — تقرير HTML بلا مكدّس خطوط عربي وبلا عزل ثنائي الاتجاه للأدلة · `مؤكد`:**

```rust
// crates/sysmedic-report/src/lib.rs — to_html
:root {{ color-scheme: light dark; font-family: system-ui, sans-serif; }}
```

`system-ui` وحده لا يضمن خطاً عربياً لائقاً عبر التوزيعات — بينما صفحة الموقع العربية تضبط البدائل بشكل صحيح، فالمعيار موجود في المشروع لكنه لم يُطبَّق على التقرير. والأخطر أن الأدلة (مسارات، أسماء وحدات systemd، أوامر) تُعرض في `<pre>` بلا `direction: ltr` داخل مستند `dir="rtl"`، فتُعاد ترتيب المسارات المختلطة بصرياً.

```css
/* قديم */
:root { color-scheme: light dark; font-family: system-ui, sans-serif; }
pre { overflow-x: auto; background: rgba(128,128,128,.15); padding: .6rem; border-radius: 8px; }
```

```css
/* جديد */
:root {
  color-scheme: light dark;
  font-family: system-ui, "Noto Kufi Arabic", "Noto Sans Arabic", sans-serif;
}
pre {
  overflow-x: auto; background: rgba(128,128,128,.15);
  padding: .6rem; border-radius: 8px;
  direction: ltr; text-align: left;   /* أوامر ومسارات لا تُعاد ترتيبها */
}
```

**الخصم الرابع — خاصية فيزيائية وسط خصائص منطقية · `مؤكد`:**

```css
/* docs/site/ar.html */
ul.feat { columns: 2; column-gap: 32px; padding-right: 20px; padding-left: 0; }
```

الملف نفسه يستخدم `inset-inline-end` بشكل صحيح في `.lang-switch`، فهذا تناقض داخلي لا جهل بالمفهوم.

```css
/* جديد */
ul.feat { columns: 2; column-gap: 32px; padding-inline-start: 20px; padding-inline-end: 0; }
```

**التاريخ (هجري/ميلادي):** الطوابع الزمنية RFC3339 ميلادية وتُعرَض عبر `glib::DateTime::format("%x %X")` الذي يتبع محرف اللغة (locale) — سلوك صحيح، لا ملاحظة.

**طباعة/PDF:** تصدير PDF يمرّ عبر نفس HTML، فكل ملاحظات الخط والاتجاه أعلاه تنسحب عليه.

### 3.5 إتاحة الوصول (Accessibility) — 7/10

معالجة **فوق المعتاد** لعنصر يصعب إتاحته: الخريطة الشجرية المرسومة بـ cairo غير مرئية لقارئات الشاشة، فتمت معالجتها بثلاث خطوات:

```rust
// crates/sysmedic-gui/src/disk.rs
let area = gtk::DrawingArea::builder()
    .accessible_role(gtk::AccessibleRole::Img)
    .build();
area.update_property(&[gtk::accessible::Property::Label(strings.treemap_a11y)]);
```

ثم قائمة نصية مكافئة تحمل البيانات نفسها، والنصّ العربي للتسمية يوجّه المستخدم إليها صراحةً: «خريطة شجرية لأكبر المجلدات؛ القائمة أدناه تعرض البيانات نفسها».

وتباين نصّ المربّعات **محسوب** لا مُخمَّن، بمعادلة اللمعان النسبية الصحيحة، مع اختبار يحرسه:

```rust
// crates/sysmedic-gui/src/disk.rs — text_color_for
let luminance = 0.2126 * bg.0 + 0.7152 * bg.1 + 0.0722 * bg.2;
if luminance > 0.55 { (0.10, 0.10, 0.12) } else { (1.0, 1.0, 1.0) }
```

وفي تقرير HTML عولج تباين شارة «متوسطة» صراحةً:

```css
/* Explicit dark text: under `color-scheme: light dark` the inherited color is
   near-white in dark mode, which fails contrast on the amber background. */
.sev-medium .badge { background: #e5a50a; color: #241f31; }
```

حساب فعلي للتباين على هذه الحالة: `#241f31` على `#e5a50a` ≈ **8.5:1** — يتجاوز AAA للنص العادي. وشارة `.sev-critical` (`#fff` على `#c01c28`) ≈ **6.4:1** — يتجاوز AA بوضوح.

**الخصم الأول — شارة الخطورة المنخفضة بلا لون نصّ صريح · `محتمل`:**

```css
.badge { font-size: .7rem; ...; background: rgba(128,128,128,.25); }
```

`.sev-low` و`.sev-info` لا تتجاوزان `.badge`، فتعتمدان على اللون الموروث تحت `color-scheme: light dark` — وهي بالضبط الحالة التي عُولجت لـ `.sev-medium` وتُركت هنا. لم أستطع حساب نسبة قاطعة لأن اللون الموروث يعتمد على متصفّح المستخدم وثيمه، وهذا بذاته هو المشكلة. أصنّفه `محتمل` لأن التحقق يحتاج عرضاً فعلياً.

**الخصم الثاني — لا احترام لـ `prefers-reduced-motion` · `مؤكد`.** المشروع يستخدم `gtk::Spinner` دائم الدوران أثناء الفحص (الذي قد يستغرق عشرات الثواني) بلا أي فحص للتفضيل. لا `prefers-reduced-motion` في أي من `docs/site/*.html` أيضاً (تحقّقت بالبحث).

**الخصم الثالث — الخريطة الشجرية غير قابلة للتنقّل بلوحة المفاتيح · `مؤكد`.** `DrawingArea` بلا `focusable`، فالمستخدم الذي يعتمد لوحة المفاتيح يصل إلى القائمة النصية فقط. هذا تخفيف مقبول لا حلٌّ كامل، وهو أفضل من لا شيء لكنه يمنع الوصول لدرجة 8.

### 3.6 تغذية المستخدم الراجعة — 8/10

هذا المحور من أقوى ما في المشروع، وأربعة أدلة تبرّر الدرجة:

**(أ) حالات فارغة صادقة لا شاشات بيضاء.** عندما ينتهي المسح بنتيجة صفرية:

```rust
// crates/sysmedic-gui/src/disk.rs
// An empty result usually means the folder was unreadable or genuinely
// empty — either way, say so instead of leaving the "Scanning…" subtitle
// up forever over a blank canvas.
Ok(node) if node.size == 0 || node.children.is_empty() => {
    subtitle.set_text(strings.disk_empty);
}
```

**(ب) الفشل يُقال، لا يُبتلع.** بعد محاولة إصلاح فاشلة أو مصادقة ملغاة:

```rust
// crates/sysmedic-gui/src/ui.rs — confirm_and_apply
// Tell the user why nothing changed instead of a silent re-scan
// (auth cancelled, fix failed, or the helper isn't installed).
let error = adw::AlertDialog::new(
    Some(strings.fix_failed_title), Some(strings.fix_failed_body));
```

**(ج) التمييز البصري بين القابل للتراجع وغير القابل.**

```rust
// An irreversible privileged change earns the destructive (red) style,
// not the encouraging blue "suggested" one.
dialog.set_response_appearance("apply",
    if plan.reversible { adw::ResponseAppearance::Suggested }
    else { adw::ResponseAppearance::Destructive });
dialog.set_default_response(Some("cancel"));
```

الافتراضي هو «إلغاء» — القرار الصحيح لحوار يطلب صلاحيات root.

**(د) بوابة مزدوجة في الطرفية:** `--dry-run` للمعاينة، ثم `--yes` للتنفيذ، ولا شيء يحدث بينهما.

**الخصم:** مسح القرص بلا شريط تقدّم ولا زر إلغاء (ذُكر في 2.5 و3.2) — وهي العملية الوحيدة الطويلة في التطبيق، وهي بالضبط العملية التي لا تملك تغذية راجعة تقدّمية.

### متوسط فئة UX/UI

(8 + 7 + 8 + 6 + 7 + 8) ÷ 6 = **7.3/10**

---

## 4. المراجعة الأمنية

**نموذج التهديد المعتمد في التقييم:** أداة سطح مكتب محلية، بلا شبكة واردة، بلا مصادقة متعددة المستخدمين. المدخلات غير الموثوقة الحقيقية هي: (1) بيانات النظام التي قد يؤثّر فيها مستخدم محلي آخر (أسماء عمليات، أسماء ملفات، أسماء وحدات systemd)، (2) ردّ واجهة Claude البرمجية عند تفعيل `--deep`، (3) ملف السجل على القرص. أرفع تصنيف ما يصل إلى مسار root، وأخفّض ما لا يمكن الوصول إليه عملياً.

### 4.1 الأسرار وبيانات الاعتماد — 9/10 · `مؤكد`

فحص شجرة العمل **وتاريخ git** معاً:

```
$ git log --all --full-history -- '*.env' '*.pem' '*.key' '*credentials*' '*secret*'
(لا نتائج)
```

والبحث النصّي عن أنماط المفاتيح في كل ملفات المصدر والإعداد لم يُرجع إلا: تعليقات توثيقية، `secrets.GITHUB_TOKEN` في CI (استخدام صحيح)، وأسماء توجيهات `sshd_config`. **لا توجد أي بيانات اعتماد مسرَّبة، لا في الشجرة ولا في التاريخ.**

المفتاح الاختياري يُقرأ من البيئة فقط، ويُرفض إذا كان فارغاً، ولا يُطبع في أي مسار خطأ:

```rust
// crates/sysmedic-knowledge/src/llm.rs — from_env
let api_key = std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.trim().is_empty())?;
```

ما يُرسَل محدود ومصرَّح به: معرّف النتيجة ونصّ الدليل فقط، والتوثيق يقول ذلك في ثلاثة مواضع متطابقة (`llm.rs`, `INSTALL.md`, `ar.html`).

**النقطة المخصومة:** لا يوجد ملف `.env.example` ولا قسم يوثّق متغيّرات البيئة المؤثّرة مجتمعةً (`ANTHROPIC_API_KEY`, `SYSMEDIC_LLM_MODEL`, `SYSMEDIC_HELPER`, `XDG_STATE_HOME`, `NO_COLOR`) — أحدها (`SYSMEDIC_HELPER`) يمسّ المسار المميّز وغير موثَّق للمستخدم إطلاقاً.

### 4.2 الحقن (أوامر، XSS، مسارات، Markdown) — 9/10

**حقن الأوامر: مُغلَق بالتصميم.** لا وجود لأي `sh -c` في المستودع كله؛ كل أمر بنية `program + args`:

```rust
// crates/sysmedic-core/src/fix.rs
/// Structured (program + args, never a shell string) so it is safe to
/// execute without a shell and easy to show.
pub struct FixCommand { pub program: String, pub args: Vec<String> }
```

و`PATH` مُثبَّت في كلا منفّذَي الأوامر لمنع استبدال ثنائي:

```rust
// crates/sysmedic-fixes/src/command.rs
const SAFE_PATH: &str = "/usr/sbin:/usr/bin:/sbin:/bin";
```

حتى استدعاء `notify-send` محصَّن ضد عنوان يبدأ بشرطة:

```rust
// crates/sysmedic-cli/src/tools.rs — notify
// `--` so a title that ever starts with `-` cannot become an option.
"--", &alert.title, &alert.body,
```

**XSS في تقرير HTML: مُغلَق ومُختبَر.** دالة `esc` تغطي سياق العنصر **والسمة** معاً (بما فيها علامتا الاقتباس)، ويحرسها اختبار `html_escapes_injection_in_findings`.

**حقن Markdown: معالَج بوعي نادر.** المشروع أدرك أن تقارير Markdown تُلصق في issues على GitHub:

```rust
// crates/sysmedic-report/src/lib.rs — md_inline
/// Finding titles, summaries and evidence contain attacker-influenceable
/// process/file/service names, and Markdown reports are pasted into
/// GitHub issues/wikis where raw inline HTML and Markdown metacharacters
/// are rendered.
```

**حقن تسلسلات الطرفية: معالَج جزئياً.** `text::sanitize` تُزيل محارف التحكّم وتُطبَّق على العناوين والملخصات والأدلة وردّ النموذج اللغوي.

**الملاحظة الوحيدة — أسماء الملفات في `sysmedic disk` غير مُنقّاة · `مؤكد` · 🔵 منخفض:**

```rust
// crates/sysmedic-cli/src/tools.rs — disk
let name = if child.is_dir { format!("{}/", child.name) } else { child.name.clone() };
println!("  {bar}  {:>10}  {name}", human(child.size));
```

`tools.rs` لا يستورد `text::sanitize` أصلاً. سيناريو الفشل: مستخدم محلي آخر (أو أرشيف مفكوك) يُنشئ ملفاً باسم يحوي `\x1b[2J\x1b[H` داخل مجلد يفحصه الضحية بـ `sysmedic disk /shared`؛ عند الطباعة تُمسح الشاشة ويمكن تزوير سطور ناتج. الأثر تجميلي/تضليلي لا تنفيذي، ولذلك 🔵 منخفض لا أعلى — لكن الإصلاح سطر واحد والدالة موجودة بالفعل.

```rust
// قديم
println!("  {bar}  {:>10}  {name}", human(child.size));
```

```rust
// جديد
println!("  {bar}  {:>10}  {}", human(child.size), crate::text::sanitize(&name));
```

الملاحظة نفسها تنطبق على `collection_errors` في `text::render` (تُطبع بلا `sanitize`)، وإن كانت رسائلها ثابتة مبرمَجة حالياً — فهي فخّ صيانة أكثر منها ثغرة.

### 4.3 التحقق من المدخلات والتطهير — 8/10

التحقق يقع عند الحدّ الصحيح تماماً: **قبل أي عمل بصلاحيات root**، والاختبارات توثّق نوايا المهاجم صراحةً:

```rust
// crates/sysmedic-fix-helper/src/main.rs — tests
assert!(parse_action(&args(&["apply", "fix.enable_ufw; rm -rf /"])).is_err());
assert!(parse_action(&args(&["apply", "../../etc/passwd"])).is_err());
```

كذلك تحليل `/proc/net/tcp` يتعامل مع الترميز little-endian بشكل صحيح ويصنّف العناوين عبر `std::net::Ipv6Addr` بدل مطابقة نصية — مع اختبار انحدار موثَّق لحالة `::ffff:127.0.0.1` التي كانت تُصنَّف خطأً كمكشوفة.

وتحليل `sshd_config` يتبع دلالات sshd الفعلية لا الحدسية: **أول** قيمة تفوز لا الأخيرة، والتحليل يتوقف عند أول كتلة `Match`:

```rust
// crates/sysmedic-collectors/src/security.rs — parse_directive_bool
if key.eq_ignore_ascii_case("match") { break; }
```

**الخصم:** `smartctl` يُستدعى بمسار جهاز مشتقّ من ناتج `smartctl --scan -j` نفسه دون التحقق من أنه لا يبدأ بشرطة:

```rust
// crates/sysmedic-collectors/src/smart.rs
match util::run_captured("smartctl", &["-j", "-H", "-A", "-i", &dev]) {
```

المصدر موثوق عملياً (smartctl نفسه)، فأصنّفها `محتمل` وأثرها نظري — لكن إضافة `--` قبل `&dev` تُغلق الباب بلا تكلفة.

### 4.4 المصادقة والتفويض — 8/10

التصميم صحيح: التطبيق **لا يعمل كـ root أبداً**؛ يستبدل نفسه بـ `pkexec` الذي يستشير polkit:

```rust
// crates/sysmedic-cli/src/fix.rs — delegate
let err = Command::new("pkexec").arg(&helper).args(args).exec();
```

وسياسة polkit تطلب مصادقة إدارية في الحالات الثلاث بلا تخزين مؤقت (`auth_admin` لا `auth_admin_keep`)، وتُقيّد المسار التنفيذي:

```xml
<annotate key="org.freedesktop.policykit.exec.path">/usr/libexec/sysmedic-fix-helper</annotate>
```

والمساعد يرفض العمل إن لم يكن root بالفعل — دفاع في العمق ضد الاستدعاء المباشر.

**الملاحظة — `SYSMEDIC_HELPER` مقبض غير ضروري في المسار المميّز · `مؤكد` · 🟡 متوسط:**

```rust
// crates/sysmedic-cli/src/fix.rs و crates/sysmedic-gui/src/ui.rs
fn helper_path() -> String {
    std::env::var("SYSMEDIC_HELPER").unwrap_or_else(|_| DEFAULT_HELPER.to_string())
}
```

المتغيّر يُمرَّر مباشرةً إلى `pkexec`. **هذا ليس تصعيد صلاحيات**: ثنائي في مسار آخر لن يطابق `exec.path` في سياسة SysMedic، فيسقط إلى الإجراء العام `org.freedesktop.policykit.exec` الذي يطلب مصادقة إدارية على أي حال — والمهاجم القادر على ضبط بيئة الجلسة يستطيع استدعاء `pkexec` مباشرةً. لكن الأثر الواقعي هو **تضليل**: التطبيق يطبع «Requesting authorization…» ويظهر حوار polkit يعرض ثنائياً اختاره المهاجم، فيوافق المستخدم وهو يظن أنه يوافق على SysMedic.

```rust
// قديم
fn helper_path() -> String {
    std::env::var("SYSMEDIC_HELPER").unwrap_or_else(|_| DEFAULT_HELPER.to_string())
}
```

```rust
// جديد — التجاوز لأغراض التطوير فقط، ومعطَّل في بناء الإصدار
fn helper_path() -> String {
    #[cfg(debug_assertions)]
    if let Ok(p) = std::env::var("SYSMEDIC_HELPER") {
        return p;
    }
    DEFAULT_HELPER.to_string()
}
```

### 4.5 التعامل مع البيانات الحسّاسة — 7/10

ما هو صحيح: التقارير تُكتب بصلاحيات المالك فقط مع تعليق يبرّر ذلك («hostnames, listening ports and the package inventory — useful recon data»)؛ سجل المعاملات يُكتب `0600` داخل دليل `0700` عبر إعادة تسمية ذرّية مع `O_NOFOLLOW` لإغلاق نافذة TOCTOU؛ وتصدير PDF استُبدل فيه المسار الثابت المتوقَّع بملف مؤقّت خاص:

```rust
// crates/sysmedic-report/src/lib.rs — write_pdf
// This avoids the fixed, predictable, world-readable path in the shared
// temp dir, which was open to a symlink-overwrite attack and leaked the
// full report to other local users.
```

**الثغرة الأولى — `mode()` لا يُطبَّق على ملف موجود · `مؤكد` · 🟠 عالي.** هذه أهم ملاحظة أمنية عملية في المراجعة:

```rust
// crates/sysmedic-cli/src/main.rs — write_private (ونظيرتها في gui/src/ui.rs)
let mut f = fs::OpenOptions::new()
    .write(true)
    .create(true)
    .truncate(true)
    .mode(0o600)
    .open(path)?;
```

في دلالات Unix، الوسيط `mode` يُستخدم **عند إنشاء الملف فقط**؛ إن كان الملف موجوداً تُتجاهَل القيمة بالكامل وتبقى صلاحياته الأصلية.

سيناريو الفشل الملموس: المستخدم ينفّذ `sysmedic checkup --format html --output ~/report.html`، فيُنشأ الملف بـ `0600` ✓. لاحقاً يفتحه بمحرّر يكتب عبر ملف مؤقّت + إعادة تسمية (وهو سلوك شائع في `vim` و`gedit`)، فيصير الملف `0644` حسب umask. ثم يعيد المستخدم توليد التقرير بالأمر نفسه — فيُكتب المحتوى الجديد الكامل (أسماء المضيف، المنافذ المستمعة، جرد الحزم، مسارات ملفات السجلات) في ملف **مقروء لكل مستخدمي الجهاز**، بينما الشيفرة والتعليق يؤكّدان عكس ذلك. الأمر ينطبق حرفياً على مسار التصدير في الواجهة الرسومية، حيث يختار المستخدم عبر `FileDialog` ملفاً قد يكون موجوداً أصلاً.

```rust
// جديد — افرض الصلاحيات بعد الفتح لا عند الإنشاء فقط
let mut f = fs::OpenOptions::new()
    .write(true).create(true).truncate(true)
    .mode(0o600)
    .open(path)?;
f.set_permissions(fs::Permissions::from_mode(0o600))?;   // يصحّح ملفاً موجوداً
f.write_all(contents.as_bytes())
```

**الثغرة الثانية — الرجوع إلى `/tmp` عند غياب `HOME` · `مؤكد` · 🔵 منخفض.** موضعان متطابقان:

```rust
// crates/sysmedic-history/src/lib.rs — default_path
// (ونظيرتها في crates/sysmedic-fixes/src/lib.rs — journal_path)
.unwrap_or_else(|| PathBuf::from("/tmp"));
base.join("sysmedic/history.jsonl")
```

وسجلّ التاريخ — بخلاف سجل المعاملات — يُفتح **بلا `O_NOFOLLOW`**:

```rust
// crates/sysmedic-history/src/lib.rs — append
opts.create(true).append(true);
opts.mode(0o600);
```

سيناريو الفشل: عملية تعمل بلا `HOME` ولا `XDG_STATE_HOME` (خدمة نظام، حاوية بسيطة، `env -i`). مستخدم محلي آخر ينشئ `/tmp/sysmedic` مسبقاً — و`create_dir_all` تنجح صامتةً لأن الدليل موجود — ويزرع `history.jsonl` كوصلة رمزية إلى ملف يملكه الضحية. عندئذ يُلحِق SysMedic أسطر JSON بذلك الملف. الأثر محدود (المحتوى JSON لا يُنفَّذ، ويتطلب غياب `HOME` وهو نادر)، فيبقى 🔵 منخفض — لكن الإصلاح موجود جاهزاً في `journal.rs` ويكفي تعميمه:

```rust
// جديد — crates/sysmedic-history/src/lib.rs
use std::os::unix::fs::OpenOptionsExt as _;
opts.create(true).append(true).mode(0o600).custom_flags(libc::O_NOFOLLOW);
```

مع الأفضل: الفشل الصريح بدل السقوط إلى `/tmp` عند غياب كلا المتغيّرين.

### 4.6 صحّة التبعيات — 7/10

**ما هو صحيح، وهو فوق المعتاد بوضوح:** `Cargo.lock` مُتتبَّع؛ `dependabot.yml` يغطي `cargo` **و**`github-actions` أسبوعياً؛ كل GitHub Action مثبَّتة على commit SHA لا على وسم متغيّر؛ وبوابة تدقيق مستقلة في CI:

```yaml
# .github/workflows/ci.yml
  audit:
    name: Dependency advisories
    steps:
      - uses: rustsec/audit-check@69366f33c96575abad1ee0dba8212993eecbe998 # v2.0.0
```

مع تعليق يبرّرها: «which matters for a binary that runs privileged fixes».

**الملاحظة — تبعية معلَنة الهجر · `مؤكد` · 🟡 متوسط.** الإصدار نفسه يحمل الإعلان في سلسلته:

```
# Cargo.lock
name = "serde_yaml"
version = "0.9.34+deprecated"
```

المشروع الأعلى أعلن نهاية صيانته، فلن تصله إصلاحات علل مستقبلية. **لا أذكر أي رقم CVE هنا لأنني لا أستطيع التحقق منه في هذه البيئة** — التوصيف الصحيح هو: *إصدار مهجور، يحتاج فحص advisories عبر `cargo audit`.* سطح الخطر محدود واقعياً لأن المُدخل الوحيد المُحلَّل هو `knowledge.yaml` المضمَّن في الثنائي عبر `include_str!` (أي مُدخل موثوق بالكامل، لا يأتي من المستخدم)، ولذلك 🟡 لا أعلى. البديلان المطروحان عادةً هما `serde_yaml_ng` أو `serde_yml`، وكلاهما يحتاج تقييماً مستقلاً قبل الاعتماد.

ملاحظتان أخف: `ureq` على الخط 2.x بينما صدر 3.x (فرق major لا ثغرة)، و`cargo update --dry-run` أبلغ عن **10 تبعيات خلف أحدث إصدار** ضمن قيود التوافق الحالية.

**لم أتمكّن من تشغيل `cargo audit` محلياً** — الأداة غير مثبَّتة في بيئة المراجعة، وتثبيت تبعيات جديدة خارج نطاق مراجعة القراءة فقط:

```
$ cargo audit --version
error: no such command: `audit`
```

لذلك لا أزعم وجود أو غياب advisories نشطة؛ بوابة CI هي المرجع.

### 4.7 CORS ورؤوس الأمان — `لا ينطبق`

لا يوجد أي مكوّن خادم أو نقطة نهاية HTTP واردة. الاتصال الشبكي الوحيد صادر (`ureq` عبر rustls مع مهلتين صريحتين)، وتقرير HTML ملف محلي محتواه مُهرَّب بالكامل، وصفحة الموقع محتوى ثابت على GitHub Pages. **تُستبعد هذه الفئة من حساب المتوسط** بدل تصفيرها.

### متوسط الفئة الأمنية

(9 + 9 + 8 + 8 + 7 + 7) ÷ 6 = **8.0/10**

---

## 5. الدرجة الإجمالية المرجّحة

ملف الأوزان المختار: **A — تطبيق بواجهة مستخدم** (هندسة 45% / UX 30% / أمن 25%).

| الناحية | الدرجة | الوزن | المساهمة |
|---|---|---|---|
| تقنية وهندسية | 7.9/10 | 45% | 3.555 |
| UX/UI | 7.3/10 | 30% | 2.190 |
| أمن سيبراني | 8.0/10 | 25% | 2.000 |
| **الإجمالي** | | **100%** | **7.7/10** |

**الحساب:** (7.9 × 0.45) + (7.3 × 0.30) + (8.0 × 0.25) = 3.555 + 2.190 + 2.000 = **7.745 ≈ 7.7/10**

**قراءة الدرجة مقابل مرتكزات التقييم:** النطاق 7-8 يعني «درجة إنتاجية: بنية متعمَّدة، معالجة أخطاء، اختبارات، توثيق حقيقي» — وهذا ينطبق حرفياً. المشروع فوق المتوسط السائد (5-6) بفارق واضح ومُبرَّر بأدلة، ولم يبلغ 9 لأن الدَّين المتبقّي حقيقي: فجوات تعريب في مسار الموافقة على تغيير مميّز، وثغرة صلاحيات ملفات قابلة لإعادة الإنتاج، وثلاث من أربع صيغ تعبئة لا تعمل فيها الوظيفة المُعلَنة الأساسية.

---

## 6. خطة الإصلاح المرتبة بالأولوية

| # | الأولوية | الناحية | المشكلة | الملف | الحل المقترح | الجهد | التأثير بعد الحل |
|---|---|---|---|---|---|---|---|
| 1 | حرج 🔴 | أمن | `mode(0o600)` يُتجاهَل على ملف موجود → تقرير يحوي بيانات استطلاع يبقى مقروءاً للجميع | `sysmedic-cli/src/main.rs`، `sysmedic-gui/src/ui.rs` | `f.set_permissions(0o600)` بعد `open` | دقائق | إغلاق تسريب بيانات محلي قابل لإعادة الإنتاج |
| 2 | حرج 🔴 | هندسة | `apply` ينفّذ ثم يسجّل → فشل الكتابة يترك إصلاحاً مُطبَّقاً وغير قابل للتراجع | `sysmedic-fixes/src/lib.rs` | تسجيل «معلَّق» قبل التنفيذ ثم تأكيده، أو خطأ صريح يقول إن الإصلاح نُفِّذ | ساعة | `undo` يصبح موثوقاً في حالة امتلاء القرص |
| 3 | مهم 🟠 | UX/تعريب | عنوان ووصف خطة الإصلاح إنجليزيان في حوار الموافقة العربي على تغيير مميّز | `sysmedic-core/src/fix.rs`، `sysmedic-fixes/src/fixes.rs` | `title_in(lang)`/`description_in(lang)` على `Fix`، أو قوالب في `knowledge.yaml` بمفتاح `fix.*` | يوم | موافقة مستنيرة فعلية للمستخدم العربي |
| 4 | مهم 🟠 | UX/تعريب | تنبيهات سطح المكتب إنجليزية بالكامل بلا معامل لغة | `sysmedic-core/src/alert.rs` | `evaluate(snapshot, lang)` + جدول تسميات كالموجود في `finding.rs` | ساعة | ركن «المتابعة» يفي بوعد التعريب |
| 5 | مهم 🟠 | أمن | `SYSMEDIC_HELPER` يوجّه `pkexec` إلى ثنائي عشوائي → حوار مصادقة مضلِّل | `sysmedic-cli/src/fix.rs`، `sysmedic-gui/src/ui.rs` | حصر التجاوز بـ `#[cfg(debug_assertions)]` | دقائق | حوار polkit يعرض دائماً الثنائي المتوقَّع |
| 6 | مهم 🟠 | UX | لا شريط تقدّم ولا إلغاء لمسح القرص (العملية الطويلة الوحيدة) | `sysmedic-gui/src/disk.rs`، `sysmedic-diskscan/src/lib.rs` | تمرير `Cancellable` + بثّ تقدّم عبر قناة | يوم | التطبيق لا يبدو معلَّقاً على مجلد منزل كبير |
| 7 | مهم 🟠 | UX | صفحة القرص تفحص `$HOME` ثابتاً، بلا اختيار مجلد ولا تعمّق في المربّعات | `sysmedic-gui/src/disk.rs` | `FileDialog::select_folder` + `GestureClick` على `DrawingArea` | يوم | يبلغ محلّل القرص التكافؤ مع نظيره في الطرفية |
| 8 | مهم 🟠 | أمن/تبعيات | `serde_yaml 0.9.34+deprecated` مهجور من مطوّره | `Cargo.toml`، `Cargo.lock` | تقييم `serde_yaml_ng`/`serde_yml` والانتقال | ساعة | إزالة تبعية بلا صيانة من مسار البناء |
| 9 | مهم 🟠 | توثيق | عدد القواعد خطأ في 5 ملفات (27 و21 مقابل 28 فعلية) | `README.md`، `docs/site/{index,ar}.html`، `docs/ISSUES.md`، `docs/ROADMAP.md` | تصحيح الأرقام + اختبار يحرس `FINDING_IDS.len()` | دقائق | التوثيق يتوقف عن التناقض مع الشيفرة |
| 10 | مهم 🟠 | UX/تعريب | تقرير HTML بلا مكدّس خطوط عربي وبلا `direction: ltr` على `<pre>` | `sysmedic-report/src/lib.rs` | إضافة `Noto Kufi Arabic`/`Noto Sans Arabic` + عزل الأدلة إلى LTR | دقائق | تقارير ومخرجات PDF عربية مقروءة |
| 11 | مهم 🟠 | UX | التطبيق يُشحن بلا أيقونة رغم `Icon=` في `.desktop` و`application_icon` في حوار «عن» | `data/`، `packaging/deb/build-deb.sh` | إضافة أصل SVG/PNG وتثبيته في `hicolor` | ساعة | التطبيق يظهر بهوية بصرية في القائمة والشريط |
| 12 | مهم 🟠 | هندسة | 5 مواضع تكرار: `helper_path`، `SAFE_PATH`، `write_private`، استنتاج XDG، `FIX_IDS` | 8 ملفات عبر 5 crates | ترقية المشترك إلى `sysmedic-core`/`sysmedic-fixes` | ساعة | تشديد المسار الأمني في موضع واحد يسري على الجميع |
| 13 | تحسين 🟡 | أمن | أسماء الملفات في `sysmedic disk` تُطبع بلا `sanitize` → حقن تسلسلات طرفية | `sysmedic-cli/src/tools.rs` | استدعاء `text::sanitize` (الدالة موجودة) | دقائق | إغلاق تزوير مخرجات الطرفية |
| 14 | تحسين 🟡 | UX/إتاحة | `.sev-low`/`.sev-info` بلا لون نصّ صريح تحت `color-scheme: light dark` | `sysmedic-report/src/lib.rs` | لون صريح كما فُعل لـ `.sev-medium` | دقائق | تباين مضمون في الوضعين |
| 15 | تحسين 🟡 | UX/إتاحة | لا احترام لـ `prefers-reduced-motion` (spinner دائم أثناء فحص طويل) | `sysmedic-gui/src/ui.rs`، `docs/site/*.html` | استبدال الدوران بمؤشّر ثابت عند تفعيل التفضيل | ساعة | راحة مستخدمي حساسية الحركة |
| 16 | تحسين 🟡 | UX/إتاحة | مربّعات الخريطة الشجرية غير قابلة للتركيز بلوحة المفاتيح | `sysmedic-gui/src/disk.rs` | `set_focusable(true)` + تنقّل بالأسهم بين المربّعات | يوم | تكافؤ فعلي لا قائمة بديلة فقط |
| 17 | تحسين 🟡 | أمن | `history::append` بلا `O_NOFOLLOW`، والرجوع إلى `/tmp` عند غياب `HOME` | `sysmedic-history/src/lib.rs`، `sysmedic-fixes/src/lib.rs` | `custom_flags(O_NOFOLLOW)` + فشل صريح بدل `/tmp` | دقائق | إغلاق مسار وصلة رمزية في حالة حافّة |
| 18 | تحسين 🟡 | UX/RTL | `padding-right`/`padding-left` فيزيائية وسط خصائص منطقية | `docs/site/ar.html` | `padding-inline-start`/`padding-inline-end` | دقائق | اتساق داخلي في نمط RTL |
| 19 | تحسين 🟡 | هندسة | `ui.rs` بلغ 561 سطراً و`build_window` تتجاوز 200 سطر | `sysmedic-gui/src/ui.rs` | استخراج تسجيل الإجراءات ومنطق التصدير إلى دوال | ساعة | تقليل مخاطر أكثر ملفات المصدر تغيّراً |
| 20 | تحسين 🟡 | هندسة | `fixes::find` تعيد بناء السجل كاملاً في كل نداء | `sysmedic-fixes/src/fixes.rs` | `once_cell::Lazy<Vec<Box<dyn Fix>>>` مع `&'static dyn Fix` | ساعة | إزالة تخصيصات متكرّرة في مسار عرض النتائج |
| 21 | تحسين 🟡 | هندسة | بادئة المعرّف تخالف الفئة في `packages.security_updates` و`snap.old_revisions` | `sysmedic-diagnostics/src/rules.rs` | إعادة تسمية (تغيير كاسر — يستلزم ترحيل `knowledge.yaml`) | ساعة | اتساق يسمح بالتجميع البرمجي حسب الفئة |
| 22 | تحسين 🟡 | هندسة | لا اختبارات لربط ودجات الواجهة، ولا قياس تغطية في CI | `sysmedic-gui/src/`، `.github/workflows/ci.yml` | فصل منطق `confirm_and_apply` إلى دالة نقية + `cargo llvm-cov` | يوم | تغطية مسار قرار التشغيل المميّز |
| 23 | تحسين 🟡 | توثيق | `INSTALL.md` يوجّه إلى Flatpak/Snap/AppImage بلا تحذير أن الإصلاحات معطّلة فيها | `INSTALL.md` | نقل التحذير من `packaging/README.md` | دقائق | توقّعات صحيحة قبل التثبيت |
| 24 | تحسين 🟡 | توثيق | لا توثيق مجمّع لمتغيّرات البيئة الخمسة المؤثّرة | `README.md` أو `INSTALL.md` | جدول واحد للمتغيّرات وأثر كلٍّ منها | دقائق | شفافية `SYSMEDIC_HELPER` تحديداً |
| 25 | تحسين 🟡 | أمن | `smartctl` يُستدعى بمسار جهاز بلا فاصل `--` | `sysmedic-collectors/src/smart.rs` | إضافة `"--"` قبل `&dev` | دقائق | تحصين نظري بتكلفة صفر |

---

## 7. أسرع المكاسب

خمسة إصلاحات، كلٌّ منها دون 30 دقيقة، وأثرها غير متناسب مع كلفتها.

### 7.1 إغلاق تسريب صلاحيات الملفات (المشكلة رقم 1)

في `crates/sysmedic-cli/src/main.rs` — دالة `write_private`:

```rust
// قديم
let mut f = fs::OpenOptions::new()
    .write(true)
    .create(true)
    .truncate(true)
    .mode(0o600)
    .open(path)?;
f.write_all(contents.as_bytes())
```

```rust
// جديد
let mut f = fs::OpenOptions::new()
    .write(true)
    .create(true)
    .truncate(true)
    .mode(0o600)
    .open(path)?;
// `mode` يسري عند الإنشاء فقط؛ افرضها صراحةً كي لا يبقى ملف موجود مسبقاً 0644.
f.set_permissions(fs::Permissions::from_mode(0o600))?;
f.write_all(contents.as_bytes())
```

والتعديل نفسه حرفياً في `crates/sysmedic-gui/src/ui.rs` — دالة `write_private`.

### 7.2 تحصين المسار المميّز من التوجيه (المشكلة رقم 5)

في `crates/sysmedic-cli/src/fix.rs` و`crates/sysmedic-gui/src/ui.rs`:

```rust
// قديم
fn helper_path() -> String {
    std::env::var("SYSMEDIC_HELPER").unwrap_or_else(|_| DEFAULT_HELPER.to_string())
}
```

```rust
// جديد — التجاوز أداة تطوير، لا سلوك إنتاجي
fn helper_path() -> String {
    #[cfg(debug_assertions)]
    if let Ok(path) = std::env::var("SYSMEDIC_HELPER") {
        return path;
    }
    DEFAULT_HELPER.to_string()
}
```

### 7.3 تقرير HTML عربي مقروء فعلاً (المشكلة رقم 10)

في `crates/sysmedic-report/src/lib.rs` — كتلة `<style>` داخل `to_html`:

```css
/* قديم */
:root { color-scheme: light dark; font-family: system-ui, sans-serif; }
pre { overflow-x: auto; background: rgba(128,128,128,.15); padding: .6rem; border-radius: 8px; }
```

```css
/* جديد */
:root {
  color-scheme: light dark;
  font-family: system-ui, "Noto Kufi Arabic", "Noto Sans Arabic", sans-serif;
}
/* الأدلة أوامر ومسارات: اعزلها عن اتجاه المستند كي لا تُعاد ترتيبها. */
pre {
  overflow-x: auto; background: rgba(128,128,128,.15);
  padding: .6rem; border-radius: 8px;
  direction: ltr; text-align: left;
}
```

### 7.4 إيقاف حقن تسلسلات الطرفية عبر أسماء الملفات (المشكلة رقم 13)

في `crates/sysmedic-cli/src/tools.rs` — دالة `disk`:

```rust
// قديم
let name = if child.is_dir {
    format!("{}/", child.name)
} else {
    child.name.clone()
};
println!("  {bar}  {:>10}  {name}", human(child.size));
```

```rust
// جديد — الدالة موجودة أصلاً في text.rs وتُستخدم في render
let name = crate::text::sanitize(&if child.is_dir {
    format!("{}/", child.name)
} else {
    child.name.clone()
});
println!("  {bar}  {:>10}  {name}", human(child.size));
```

### 7.5 حراسة عدد القواعد المعلَن (المشكلة رقم 9)

بعد تصحيح `27` → `28` في `README.md` و`docs/site/index.html` و`docs/site/ar.html`، و`21` → `28` في `docs/ISSUES.md` و`docs/ROADMAP.md`، أضف في `crates/sysmedic-diagnostics/src/lib.rs` داخل وحدة اختبار:

```rust
// جديد
#[cfg(test)]
mod doc_guards {
    #[test]
    fn declared_rule_count_matches_the_docs() {
        // عند تغيير هذا الرقم حدِّث: README.md, docs/site/index.html,
        // docs/site/ar.html, docs/ISSUES.md, docs/ROADMAP.md
        assert_eq!(super::FINDING_IDS.len(), 28);
    }
}
```

---

## 8. ما لم تتم مراجعته

**التغطية التقديرية: نحو 75% من الشيفرة المتتبَّعة، ونحو 90% من السطح الحرج** (المسار المميّز، الإصلاحات، التقارير، الشبكة، الإعداد، CI، التعبئة).

راجعتُ المستودع بأولوية: نقاط الدخول ← الشيفرة المميّزة/الشبكية/التخزينية ← الإعداد والبناء ← أكبر الملفات ← أكثرها churn.

### قُرئ بالكامل

`sysmedic-fix-helper/src/main.rs` · `sysmedic-fixes/` (الأربعة كلها) · `sysmedic-report/src/lib.rs` · `sysmedic-knowledge/src/{lib,llm}.rs` · `sysmedic-history/src/lib.rs` · `sysmedic-cli/src/{main,tools,schedule,fix,text}.rs` · `sysmedic-gui/src/{main,ui,viewmodel,disk}.rs` · `sysmedic-diskscan/src/lib.rs` · `sysmedic-core/src/{snapshot,engine,score,fix,lang,finding,alert,thresholds}.rs` · `sysmedic-collectors/src/{util,lib,security,ports}.rs` · `sysmedic-diagnostics/src/{lib,rules}.rs` · كل ملفات `Cargo.toml` و`Cargo.lock` · `.github/workflows/{ci,pages,release}.yml` · `.github/dependabot.yml` · `.gitignore` · `data/*` الأربعة · `packaging/` (السكربتان والمانيفستان و`README.md`) · `docs/site/ar.html` · `SECURITY.md` · `INSTALL.md` · `packaging/README.md`

### لم يُراجَع، والسبب

| العنصر | السبب |
|---|---|
| 12 مجمّعاً: `battery`, `boot`, `cpu`, `disk`, `flatpak`, `logs`, `memory`, `network`, `process`, `services`, `snap`, `thermal` | نمط متجانس مُتحقَّق منه في العيّنة المقروءة (`security`, `ports`, `util`, وأجزاء من `packages`, `smart`): فصل I/O عن التحليل، ولا استدعاء صدفة. تغطيهم 53 اختباراً ناجحاً. لم أفحص منطق التحليل سطراً بسطر — قد تختبئ فيه أخطاء تحليل حسابية |
| `sysmedic-collectors/src/packages.rs` (جزئياً) و`smart.rs` (جزئياً) | قرأتُ الكشف والاستدعاءات الخارجية؛ لم أراجع دوال التحليل التفصيلية لكل مدير حزم |
| `sysmedic-diskscan/src/treemap.rs` (جزئياً) | قرأتُ `squarify` و`worst_ratio`؛ لم أتحقق من صحة `layout_row` رياضياً ولا من خوارزمية Bruls et al. مقابل الورقة الأصلية |
| `crates/sysmedic-knowledge/data/knowledge.yaml` (39 كيلوبايت، 28 مدخلاً ثنائي اللغة) | لم تُراجَع **جودة النصّ العربي** لأي شرح (صحة لغوية، دقة المصطلح التقني، ملاءمة النبرة). حرست الاختبارات **الوجود** والقوالب فقط، لا الجودة. هذه فجوة حقيقية في تقييم محور التعريب وتحتاج مراجعاً لغوياً |
| `docs/{ARCHITECTURE,VISION,ROADMAP,COMPETITIVE-ANALYSIS}.md` و`ENGINEERING-REVIEW.md` (41 كيلوبايت) و`CHANGELOG.md` و`CONTRIBUTING.md` | وثائق سردية لا تؤثّر على السلوك؛ استخدمتُ `ISSUES.md` و`ROADMAP.md` للتحقق من الأرقام فقط |
| `docs/screenshots/*.png` (5 ملفات) و`docs/site/index.html` (جزئياً) | ملفات ثنائية لا تُراجَع نصياً؛ قرأتُ ادعاءات الأرقام وروابط تبديل اللغة في `index.html` |
| مجلد `target/` | مُستبعَد عبر `.gitignore` — نواتج بناء |

### لم يُتحقَّق منه تشغيلياً

| العنصر | السبب |
|---|---|
| **`sysmedic-gui` لم يُبنَ ولا اختباراته شُغّلت** | GTK4 غير مثبَّت في بيئة المراجعة: `pkg-config --modversion gtk4` → `Package gtk4 was not found`. **كل ملاحظات الواجهة الرسومية مستندة إلى قراءة الشيفرة فقط، بلا تحقق بصري أو تشغيلي.** هذا يشمل تقييمات الاستجابة والتباين والإتاحة |
| **`cargo audit`** | `error: no such command: audit` — والأداة تحتاج تثبيتاً، وهو خارج نطاق مراجعة القراءة فقط. لم أزعم وجود ولا غياب advisories نشطة، ولم أذكر أي رقم CVE |
| **`clippy` على `sysmedic-gui`** | استُثني للسبب نفسه؛ نتيجة «نظيف» تخصّ 10 من 11 crate. CI يشغّله على الحزمة كاملة |
| **سلوك polkit/pkexec الفعلي** | يحتاج نظاماً حياً بـ polkit وجلسة رسومية. تحليل المسارات (`exec.path` مقابل ما تثبّته كل صيغة تعبئة) مستند إلى قراءة `.policy` وسكربتات التعبئة، وهذا كافٍ للتناقض الذي أبلغتُ عنه |
| **حزم `.deb`/Flatpak/Snap/AppImage** | لم تُبنَ ولم تُثبَّت |
| **مسار `--deep`** | لا مفتاح `ANTHROPIC_API_KEY`؛ التحقق تمّ عبر الاختبارات الستة التي تستخدم `FakeTransport` |

### مستوى الثقة

- **عالٍ** في ملاحظات الأمن والهندسة على الشيفرة المقروءة: كلها مسنودة باقتباس مباشر من ملف فُتح فعلاً، ومصنَّفة `مؤكد` أو `محتمل` صراحةً.
- **عالٍ** في أرقام الاختبارات والأدوات: نواتج أوامر حقيقية أُدرجت كما ظهرت.
- **متوسط** في تقييم الواجهة الرسومية: قراءة شيفرة بلا تشغيل. ملاحظات مثل «التباين مقبول» و«الاستجابة جيدة» مستنتجة من الشيفرة والقيم اللونية لا من عرض فعلي.
- **منخفض** في جودة النصّ العربي داخل قاعدة المعرفة: لم يُراجَع إطلاقاً. درجة محور التعريب (6/10) تقيس **آلية** التعريب وتغطيتها، لا **جودة** الترجمة.

---

*مراجعة قراءة فقط — لم يُعدَّل أو يُنشأ أو يُحذف أي ملف في المستودع عدا هذا التقرير، ولم تُنفَّذ أي عملية git تعديلية.*
