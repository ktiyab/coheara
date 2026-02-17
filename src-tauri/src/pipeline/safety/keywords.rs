use std::sync::LazyLock;

use regex::Regex;

use super::types::{FilterLayer, Violation, ViolationCategory};

/// N.4: Supported languages for safety scanning.
pub const SUPPORTED_LANGUAGES: &[&str] = &["en", "fr", "de"];

/// Check if a language code is supported.
pub fn is_supported_language(lang: &str) -> bool {
    SUPPORTED_LANGUAGES.contains(&lang)
}

/// A compiled pattern with its violation metadata.
struct SafetyPattern {
    regex: Regex,
    category: ViolationCategory,
    description: &'static str,
}

// ═══════════════════════════════════════════════════════════
// N.3: ACCENT NORMALIZATION — strip diacritics for matching
// ═══════════════════════════════════════════════════════════

/// Strip common French/German diacritics to ASCII for safety pattern matching.
/// This enables accent-insensitive matching without requiring NFC↔NFD normalization.
fn strip_accents(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => result.push(if ch.is_uppercase() { 'E' } else { 'e' }),
            'à' | 'â' | 'À' | 'Â' => result.push(if ch.is_uppercase() { 'A' } else { 'a' }),
            'ù' | 'û' | 'ü' | 'Ù' | 'Û' | 'Ü' => result.push(if ch.is_uppercase() { 'U' } else { 'u' }),
            'ô' | 'Ô' => result.push(if ch.is_uppercase() { 'O' } else { 'o' }),
            'ö' | 'Ö' => result.push(if ch.is_uppercase() { 'O' } else { 'o' }),
            'ä' | 'Ä' => result.push(if ch.is_uppercase() { 'A' } else { 'a' }),
            'î' | 'ï' | 'Î' | 'Ï' => result.push(if ch.is_uppercase() { 'I' } else { 'i' }),
            'ç' | 'Ç' => result.push(if ch.is_uppercase() { 'C' } else { 'c' }),
            'ß' => result.push_str("ss"),
            _ => result.push(ch),
        }
    }
    result
}

// ═══════════════════════════════════════════════════════════
// ENGLISH PATTERNS (24 total: 8 diagnostic + 8 prescriptive + 8 alarm)
// ═══════════════════════════════════════════════════════════

/// English diagnostic language patterns (Layer 2) — 8 patterns.
static EN_DIAGNOSTIC: LazyLock<Vec<SafetyPattern>> = LazyLock::new(|| {
    vec![
        pattern(
            r"(?i)\byou\s+have\s+(?:a\s+)?(?:been\s+)?(?:diagnosed\s+with\s+)?[a-z]",
            ViolationCategory::DiagnosticLanguage,
            "EN diagnostic: 'you have [condition]'",
        ),
        pattern(
            r"(?i)\byou\s+are\s+suffering\s+from\b",
            ViolationCategory::DiagnosticLanguage,
            "EN diagnostic: 'you are suffering from'",
        ),
        pattern(
            r"(?i)\byou\s+(?:likely|probably|possibly)\s+have\b",
            ViolationCategory::DiagnosticLanguage,
            "EN diagnostic: 'you likely/probably have'",
        ),
        pattern(
            r"(?i)\bthis\s+(?:means|indicates|suggests|confirms)\s+(?:you|that\s+you)\s+have\b",
            ViolationCategory::DiagnosticLanguage,
            "EN diagnostic: 'this means you have'",
        ),
        pattern(
            r"(?i)\byou\s+(?:are|have\s+been)\s+diagnosed\b",
            ViolationCategory::DiagnosticLanguage,
            "EN diagnostic: diagnosis claim without attribution",
        ),
        pattern(
            r"(?i)\byou(?:'re|\s+are)\s+(?:a\s+)?diabetic\b",
            ViolationCategory::DiagnosticLanguage,
            "EN diagnostic: 'you are diabetic'",
        ),
        pattern(
            r"(?i)\byour\s+condition\s+is\b",
            ViolationCategory::DiagnosticLanguage,
            "EN diagnostic: 'your condition is'",
        ),
        pattern(
            r"(?i)\byou\s+(?:appear|seem)\s+to\s+have\b",
            ViolationCategory::DiagnosticLanguage,
            "EN diagnostic: 'you appear to have'",
        ),
    ]
});

/// English prescriptive language patterns (Layer 2) — 8 patterns.
static EN_PRESCRIPTIVE: LazyLock<Vec<SafetyPattern>> = LazyLock::new(|| {
    vec![
        pattern(
            r"(?i)\byou\s+should\s+(?:take|stop|start|increase|decrease|change|switch|discontinue|avoid|reduce)\b",
            ViolationCategory::PrescriptiveLanguage,
            "EN prescriptive: 'you should [take/stop/...]'",
        ),
        pattern(
            r"(?i)\bI\s+recommend\b",
            ViolationCategory::PrescriptiveLanguage,
            "EN prescriptive: 'I recommend'",
        ),
        pattern(
            r"(?i)\bI\s+(?:would\s+)?(?:suggest|advise)\b",
            ViolationCategory::PrescriptiveLanguage,
            "EN prescriptive: 'I suggest/advise'",
        ),
        pattern(
            r"(?i)\byou\s+(?:need|must|have)\s+to\s+(?:take|stop|start|see|visit|go|call|increase|decrease)\b",
            ViolationCategory::PrescriptiveLanguage,
            "EN prescriptive: 'you need to [action]'",
        ),
        pattern(
            r"(?i)\bdo\s+not\s+(?:take|stop|eat|drink|use|skip)\b",
            ViolationCategory::PrescriptiveLanguage,
            "EN prescriptive: 'do not [action]'",
        ),
        pattern(
            r"(?i)\btry\s+(?:taking|using|adding|reducing)\b",
            ViolationCategory::PrescriptiveLanguage,
            "EN prescriptive: 'try taking/using'",
        ),
        pattern(
            r"(?i)\bthe\s+(?:best|recommended)\s+(?:treatment|course\s+of\s+action|approach)\s+(?:is|would\s+be)\b",
            ViolationCategory::PrescriptiveLanguage,
            "EN prescriptive: 'the best treatment is'",
        ),
        pattern(
            r"(?i)\bconsider\s+(?:taking|stopping|increasing|decreasing|switching)\b",
            ViolationCategory::PrescriptiveLanguage,
            "EN prescriptive: 'consider taking/stopping'",
        ),
    ]
});

/// English alarm/emergency language patterns (Layer 2) — 8 patterns.
static EN_ALARM: LazyLock<Vec<SafetyPattern>> = LazyLock::new(|| {
    vec![
        pattern(
            r"(?i)\b(?:dangerous|life[- ]threatening|fatal|deadly|lethal)\b",
            ViolationCategory::AlarmLanguage,
            "EN alarm: dangerous/life-threatening/fatal",
        ),
        pattern(
            r"(?i)\b(?:emergency|urgent(?:ly)?|immediately|right\s+away|right\s+now)\b",
            ViolationCategory::AlarmLanguage,
            "EN alarm: emergency/immediately/urgently",
        ),
        pattern(
            r"(?i)\b(?:immediately|urgently)\s+(?:go|call|visit|see|seek|get)\b",
            ViolationCategory::AlarmLanguage,
            "EN alarm: 'immediately go/call'",
        ),
        pattern(
            r"(?i)\bcall\s+(?:911|emergency|an\s+ambulance|your\s+doctor\s+(?:immediately|right\s+away|now))\b",
            ViolationCategory::AlarmLanguage,
            "EN alarm: 'call 911/emergency'",
        ),
        pattern(
            r"(?i)\bgo\s+to\s+(?:the\s+)?(?:emergency|ER|hospital|A&E)\b",
            ViolationCategory::AlarmLanguage,
            "EN alarm: 'go to the emergency/hospital'",
        ),
        pattern(
            r"(?i)\bseek\s+(?:immediate|emergency|urgent)\s+(?:medical\s+)?(?:help|attention|care)\b",
            ViolationCategory::AlarmLanguage,
            "EN alarm: 'seek immediate medical help'",
        ),
        pattern(
            r"(?i)\bthis\s+(?:is|could\s+be)\s+(?:a\s+)?(?:medical\s+)?emergency\b",
            ViolationCategory::AlarmLanguage,
            "EN alarm: 'this is an emergency'",
        ),
        pattern(
            r"(?i)\bdo\s+not\s+(?:wait|delay|ignore)\b",
            ViolationCategory::AlarmLanguage,
            "EN alarm: 'do not wait/delay'",
        ),
    ]
});

// ═══════════════════════════════════════════════════════════
// N.1: FRENCH PATTERNS (24 total: 8 diagnostic + 8 prescriptive + 8 alarm)
// Uses ASCII regex on accent-stripped text (see strip_accents)
// ═══════════════════════════════════════════════════════════

/// French diagnostic language patterns — 8 patterns.
/// Matched against accent-stripped text for diacritic-insensitive detection.
static FR_DIAGNOSTIC: LazyLock<Vec<SafetyPattern>> = LazyLock::new(|| {
    vec![
        pattern(
            r"(?i)\b(?:tu\s+as|vous\s+avez)\s+(?:un[e]?\s+)?",
            ViolationCategory::DiagnosticLanguage,
            "FR diagnostic: 'vous avez [condition]'",
        ),
        pattern(
            r"(?i)\b(?:tu\s+souffres|vous\s+souffrez)\s+de?\b",
            ViolationCategory::DiagnosticLanguage,
            "FR diagnostic: 'vous souffrez de'",
        ),
        pattern(
            r"(?i)\b(?:tu\s+es|vous\s+etes)\s+(?:probablement|sans\s+doute|possiblement)",
            ViolationCategory::DiagnosticLanguage,
            "FR diagnostic: 'vous etes probablement'",
        ),
        pattern(
            r"(?i)\bcela\s+(?:signifie|indique|suggere|confirme)\s+que\s+(?:tu|vous)",
            ViolationCategory::DiagnosticLanguage,
            "FR diagnostic: 'cela signifie que vous'",
        ),
        pattern(
            r"(?i)\b(?:tu\s+as|vous\s+avez)\s+ete\s+diagnostique",
            ViolationCategory::DiagnosticLanguage,
            "FR diagnostic: 'vous avez ete diagnostique'",
        ),
        pattern(
            r"(?i)\b(?:tu\s+es|vous\s+etes)\s+(?:diabetique|hypertendu|malade)",
            ViolationCategory::DiagnosticLanguage,
            "FR diagnostic: 'vous etes diabetique'",
        ),
        pattern(
            r"(?i)\bvotre\s+(?:etat|condition)\s+est\b",
            ViolationCategory::DiagnosticLanguage,
            "FR diagnostic: 'votre etat est'",
        ),
        pattern(
            r"(?i)\b(?:tu\s+sembles|vous\s+semblez)\s+avoir\b",
            ViolationCategory::DiagnosticLanguage,
            "FR diagnostic: 'vous semblez avoir'",
        ),
    ]
});

/// French prescriptive language patterns — 8 patterns.
static FR_PRESCRIPTIVE: LazyLock<Vec<SafetyPattern>> = LazyLock::new(|| {
    vec![
        pattern(
            r"(?i)\b(?:tu\s+devrais|vous\s+devriez)\s+(?:prendre|arreter|commencer|augmenter|diminuer|changer|eviter|reduire)",
            ViolationCategory::PrescriptiveLanguage,
            "FR prescriptive: 'vous devriez [prendre/arreter/...]'",
        ),
        pattern(
            r"(?i)\bje\s+(?:vous\s+)?recommande\b",
            ViolationCategory::PrescriptiveLanguage,
            "FR prescriptive: 'je recommande'",
        ),
        pattern(
            r"(?i)\bje\s+(?:vous\s+)?(?:suggere|conseille)\b",
            ViolationCategory::PrescriptiveLanguage,
            "FR prescriptive: 'je suggere/conseille'",
        ),
        pattern(
            r"(?i)\b(?:tu\s+dois|vous\s+devez|il\s+faut)\s+(?:prendre|arreter|commencer|consulter|aller|appeler)",
            ViolationCategory::PrescriptiveLanguage,
            "FR prescriptive: 'vous devez/il faut [action]'",
        ),
        pattern(
            r"(?i)\bne\s+(?:prenez|prends|mangez|buvez|utilisez)\s+(?:pas|plus)\b",
            ViolationCategory::PrescriptiveLanguage,
            "FR prescriptive: 'ne prenez pas'",
        ),
        pattern(
            r"(?i)\bessayez\s+(?:de\s+)?(?:prendre|utiliser|ajouter|reduire)",
            ViolationCategory::PrescriptiveLanguage,
            "FR prescriptive: 'essayez de prendre'",
        ),
        pattern(
            r"(?i)\ble\s+(?:meilleur|bon)\s+(?:traitement|choix|approche)\s+(?:est|serait)\b",
            ViolationCategory::PrescriptiveLanguage,
            "FR prescriptive: 'le meilleur traitement est'",
        ),
        pattern(
            r"(?i)\benvisagez\s+(?:de\s+)?(?:prendre|arreter|augmenter|diminuer|changer)",
            ViolationCategory::PrescriptiveLanguage,
            "FR prescriptive: 'envisagez de prendre'",
        ),
    ]
});

/// French alarm/emergency language patterns — 8 patterns.
static FR_ALARM: LazyLock<Vec<SafetyPattern>> = LazyLock::new(|| {
    vec![
        pattern(
            r"(?i)\b(?:dangereux|dangereuse|mortel(?:le)?|fatal[e]?|lethal)\b",
            ViolationCategory::AlarmLanguage,
            "FR alarm: dangereux/mortel/fatal",
        ),
        pattern(
            r"(?i)\b(?:urgence|urgent[e]?|immediatement|tout\s+de\s+suite)\b",
            ViolationCategory::AlarmLanguage,
            "FR alarm: urgence/immediatement",
        ),
        pattern(
            r"(?i)\b(?:immediatement|tout\s+de\s+suite)\s+(?:allez|appelez|consultez|rendez-vous)",
            ViolationCategory::AlarmLanguage,
            "FR alarm: 'immediatement allez/appelez'",
        ),
        pattern(
            r"(?i)\bappelez\s+(?:le\s+15|le\s+112|le\s+samu|une\s+ambulance|votre\s+medecin\s+(?:immediatement|tout\s+de\s+suite))\b",
            ViolationCategory::AlarmLanguage,
            "FR alarm: 'appelez le 15/112/SAMU'",
        ),
        pattern(
            r"(?i)\ballez\s+(?:aux?\s+)?(?:urgences|hopital)\b",
            ViolationCategory::AlarmLanguage,
            "FR alarm: 'allez aux urgences/hopital'",
        ),
        pattern(
            r"(?i)\b(?:cherchez|demandez)\s+(?:une?\s+)?(?:aide\s+)?(?:medicale\s+)?(?:immediate|urgente|d'urgence)",
            ViolationCategory::AlarmLanguage,
            "FR alarm: 'cherchez aide medicale immediate'",
        ),
        pattern(
            r"(?i)\bc'est\s+(?:une?\s+)?(?:urgence\s+)?medicale\b",
            ViolationCategory::AlarmLanguage,
            "FR alarm: 'c'est une urgence medicale'",
        ),
        pattern(
            r"(?i)\bn'attendez\s+pas\b",
            ViolationCategory::AlarmLanguage,
            "FR alarm: 'n'attendez pas'",
        ),
    ]
});

// ═══════════════════════════════════════════════════════════
// I18N-05: GERMAN PATTERNS (24 total: 8 diagnostic + 8 prescriptive + 8 alarm)
// Uses ASCII regex on accent-stripped text (see strip_accents: ü→u, ö→o, ä→a, ß→ss)
// Formal address "Sie" used per I18N spec (formal vous/Sie mandatory)
// ═══════════════════════════════════════════════════════════

/// German diagnostic language patterns — 8 patterns.
/// Matched against accent-stripped text for umlaut-insensitive detection.
static DE_DIAGNOSTIC: LazyLock<Vec<SafetyPattern>> = LazyLock::new(|| {
    vec![
        pattern(
            r"(?i)\bSie\s+haben\s+(?:eine?n?\s+)?",
            ViolationCategory::DiagnosticLanguage,
            "DE diagnostic: 'Sie haben [Erkrankung]'",
        ),
        pattern(
            r"(?i)\bSie\s+leiden\s+(?:an|unter)\b",
            ViolationCategory::DiagnosticLanguage,
            "DE diagnostic: 'Sie leiden an/unter'",
        ),
        pattern(
            r"(?i)\bSie\s+(?:sind\s+)?(?:wahrscheinlich|vermutlich|moglicherweise)\b",
            ViolationCategory::DiagnosticLanguage,
            "DE diagnostic: 'Sie sind wahrscheinlich'",
        ),
        pattern(
            r"(?i)\bdas\s+(?:bedeutet|zeigt|deutet|bestatigt)\s+dass\s+Sie\b",
            ViolationCategory::DiagnosticLanguage,
            "DE diagnostic: 'das bedeutet dass Sie'",
        ),
        pattern(
            r"(?i)\bbei\s+Ihnen\s+wurde\s+(?:eine?\s+)?(?:\w+\s+)?diagnostiziert\b",
            ViolationCategory::DiagnosticLanguage,
            "DE diagnostic: 'bei Ihnen wurde diagnostiziert'",
        ),
        pattern(
            r"(?i)\bSie\s+sind\s+(?:Diabetiker(?:in)?|zuckerkrank|bluthochdruck)\b",
            ViolationCategory::DiagnosticLanguage,
            "DE diagnostic: 'Sie sind Diabetiker'",
        ),
        pattern(
            r"(?i)\bIhr\s+Zustand\s+ist\b",
            ViolationCategory::DiagnosticLanguage,
            "DE diagnostic: 'Ihr Zustand ist'",
        ),
        pattern(
            r"(?i)\bSie\s+scheinen\s+(?:eine?n?\s+)?(?:\w+\s+)?zu\s+haben\b",
            ViolationCategory::DiagnosticLanguage,
            "DE diagnostic: 'Sie scheinen zu haben'",
        ),
    ]
});

/// German prescriptive language patterns — 8 patterns.
static DE_PRESCRIPTIVE: LazyLock<Vec<SafetyPattern>> = LazyLock::new(|| {
    vec![
        pattern(
            r"(?i)\bSie\s+sollten\s+(?:\w+\s+){0,5}(?:einnehmen|aufhoren|anfangen|erhohen|reduzieren|andern|vermeiden|absetzen)",
            ViolationCategory::PrescriptiveLanguage,
            "DE prescriptive: 'Sie sollten [einnehmen/aufhoren/...]'",
        ),
        pattern(
            r"(?i)\bich\s+empfehle\b",
            ViolationCategory::PrescriptiveLanguage,
            "DE prescriptive: 'ich empfehle'",
        ),
        pattern(
            r"(?i)\bich\s+(?:wurde\s+)?(?:vorschlagen|raten)\b",
            ViolationCategory::PrescriptiveLanguage,
            "DE prescriptive: 'ich schlage vor/rate'",
        ),
        pattern(
            r"(?i)\bSie\s+(?:mussen|sollen)\s+(?:\w+\s+){0,5}(?:einnehmen|aufhoren|anfangen|einen\s+Arzt|zum\s+Arzt|ins\s+Krankenhaus)",
            ViolationCategory::PrescriptiveLanguage,
            "DE prescriptive: 'Sie mussen [action]'",
        ),
        pattern(
            r"(?i)\bnehmen\s+Sie\s+(?:nicht|kein[e]?)\b",
            ViolationCategory::PrescriptiveLanguage,
            "DE prescriptive: 'nehmen Sie nicht'",
        ),
        pattern(
            r"(?i)\bversuchen\s+Sie\s+(?:\w+\s+){0,5}(?:einzunehmen|zu\s+nehmen|zu\s+verwenden|zu\s+reduzieren)",
            ViolationCategory::PrescriptiveLanguage,
            "DE prescriptive: 'versuchen Sie einzunehmen'",
        ),
        pattern(
            r"(?i)\bdie\s+beste\s+(?:Behandlung|Therapie|Massnahme)\s+(?:ist|ware)\b",
            ViolationCategory::PrescriptiveLanguage,
            "DE prescriptive: 'die beste Behandlung ist'",
        ),
        pattern(
            r"(?i)\berwagen\s+Sie\s+(?:die\s+)?(?:Einnahme|das\s+Absetzen|eine\s+Erhohung|eine\s+Reduzierung)",
            ViolationCategory::PrescriptiveLanguage,
            "DE prescriptive: 'erwagen Sie die Einnahme'",
        ),
    ]
});

/// German alarm/emergency language patterns — 8 patterns.
static DE_ALARM: LazyLock<Vec<SafetyPattern>> = LazyLock::new(|| {
    vec![
        pattern(
            r"(?i)\b(?:gefahrlich|lebensbedrohlich|todlich|letal)\b",
            ViolationCategory::AlarmLanguage,
            "DE alarm: gefahrlich/lebensbedrohlich/todlich",
        ),
        pattern(
            r"(?i)\b(?:Notfall|dringend|sofort|unverzuglich)\b",
            ViolationCategory::AlarmLanguage,
            "DE alarm: Notfall/dringend/sofort",
        ),
        pattern(
            r"(?i)\b(?:sofort|unverzuglich)\s+(?:gehen|rufen|aufsuchen|zum\s+Arzt)",
            ViolationCategory::AlarmLanguage,
            "DE alarm: 'sofort gehen/rufen'",
        ),
        pattern(
            r"(?i)\brufen\s+Sie\s+(?:die\s+)?(?:112|den\s+(?:Notarzt|Rettungsdienst)|einen\s+(?:Krankenwagen|Notarzt))\b",
            ViolationCategory::AlarmLanguage,
            "DE alarm: 'rufen Sie 112/Notarzt'",
        ),
        pattern(
            r"(?i)\bgehen\s+Sie\s+(?:in\s+die\s+)?(?:Notaufnahme|ins?\s+Krankenhaus)\b",
            ViolationCategory::AlarmLanguage,
            "DE alarm: 'gehen Sie in die Notaufnahme'",
        ),
        pattern(
            r"(?i)\bsuchen\s+Sie\s+(?:sofort(?:ige)?|dringend(?:e)?|notfall)?\s*(?:arztliche\s+)?(?:Hilfe|Behandlung)\b",
            ViolationCategory::AlarmLanguage,
            "DE alarm: 'suchen Sie sofortige Hilfe'",
        ),
        pattern(
            r"(?i)\bdies\s+ist\s+ein\s+(?:medizinischer\s+)?Notfall\b",
            ViolationCategory::AlarmLanguage,
            "DE alarm: 'dies ist ein Notfall'",
        ),
        pattern(
            r"(?i)\bwarten\s+Sie\s+nicht\b",
            ViolationCategory::AlarmLanguage,
            "DE alarm: 'warten Sie nicht'",
        ),
    ]
});

// ═══════════════════════════════════════════════════════════
// N.2: FALSE POSITIVE EXCEPTION PATTERNS
// ═══════════════════════════════════════════════════════════

/// Patterns that indicate grounded/safe text — matches exempt the surrounding
/// sentence from violation detection. Used to prevent false positives on
/// document-attributed responses.
static EXCEPTION_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // English exceptions
        Regex::new(r"(?i)\byour\s+documents?\s+(?:show|indicate|mention|state|record|note)").unwrap(),
        Regex::new(r"(?i)\b(?:according|based)\s+(?:to|on)\s+your\s+(?:records?|documents?|files?)").unwrap(),
        Regex::new(r"(?i)\b(?:is|was)\s+documented\s+(?:in|on)\b").unwrap(),
        Regex::new(r"(?i)\byour\s+doctor\s+(?:noted|recorded|documented|wrote)\b").unwrap(),
        Regex::new(r"(?i)\b(?:dr\.|doctor)\s+\w+\s+(?:noted|recorded|prescribed|documented)\b").unwrap(),
        // French exceptions
        Regex::new(r"(?i)\bvos\s+documents?\s+(?:montrent|indiquent|mentionnent)\b").unwrap(),
        Regex::new(r"(?i)\bselon\s+(?:vos\s+)?(?:documents?|dossiers?)\b").unwrap(),
        Regex::new(r"(?i)\bvotre\s+(?:medecin|docteur)\s+a\s+(?:note|prescrit|documente)\b").unwrap(),
        // German exceptions
        Regex::new(r"(?i)\bIhre\s+(?:Unterlagen|Dokumente)\s+(?:zeigen|belegen|enthalten)\b").unwrap(),
        Regex::new(r"(?i)\blaut\s+(?:Ihren?\s+)?(?:Unterlagen|Dokumenten?|Akten?|Befunden?)\b").unwrap(),
        Regex::new(r"(?i)\bIhr\s+(?:Arzt|Hausarzt)\s+hat\s+(?:festgestellt|verordnet|dokumentiert|notiert)\b").unwrap(),
    ]
});

/// Check if text at a given violation offset is within an exception context.
/// Returns true if the violation should be suppressed (false positive).
fn is_exception(text: &str, violation_offset: usize) -> bool {
    // Check a window around the violation (128 chars before, 64 chars after)
    let start = violation_offset.saturating_sub(128);
    let end = (violation_offset + 64).min(text.len());
    let window = &text[start..end];

    EXCEPTION_PATTERNS
        .iter()
        .any(|pattern| pattern.is_match(window))
}

// ═══════════════════════════════════════════════════════════
// PATTERN CONSTRUCTION + SCANNING
// ═══════════════════════════════════════════════════════════

fn pattern(regex_str: &str, category: ViolationCategory, description: &'static str) -> SafetyPattern {
    SafetyPattern {
        regex: Regex::new(regex_str).expect("Invalid safety regex pattern"),
        category,
        description,
    }
}

/// Layer 2: Scan response text for diagnostic, prescriptive, and alarm language.
/// I18N-05: Scans ALL language patterns (EN + FR + DE) on every response regardless of user language.
/// N.3: Applies accent-stripping for French/German diacritic-insensitive matching.
/// N.2: Applies exception patterns to suppress false positives from grounded text.
pub fn scan_keywords(text: &str) -> Vec<Violation> {
    let mut violations = Vec::new();

    // N.3: Create accent-stripped copy for French/German pattern matching
    let stripped = strip_accents(text);

    // Scan English patterns against original text
    for patterns in [&*EN_DIAGNOSTIC, &*EN_PRESCRIPTIVE, &*EN_ALARM] {
        for sp in patterns {
            for mat in sp.regex.find_iter(text) {
                violations.push(Violation {
                    layer: FilterLayer::KeywordScan,
                    category: sp.category.clone(),
                    matched_text: mat.as_str().to_string(),
                    offset: mat.start(),
                    length: mat.len(),
                    reason: sp.description.to_string(),
                });
            }
        }
    }

    // N.1: Scan French patterns against accent-stripped text
    for patterns in [&*FR_DIAGNOSTIC, &*FR_PRESCRIPTIVE, &*FR_ALARM] {
        for sp in patterns {
            for mat in sp.regex.find_iter(&stripped) {
                violations.push(Violation {
                    layer: FilterLayer::KeywordScan,
                    category: sp.category.clone(),
                    matched_text: mat.as_str().to_string(),
                    offset: mat.start(),
                    length: mat.len(),
                    reason: sp.description.to_string(),
                });
            }
        }
    }

    // I18N-05: Scan German patterns against accent-stripped text (ü→u, ö→o, ä→a, ß→ss)
    for patterns in [&*DE_DIAGNOSTIC, &*DE_PRESCRIPTIVE, &*DE_ALARM] {
        for sp in patterns {
            for mat in sp.regex.find_iter(&stripped) {
                violations.push(Violation {
                    layer: FilterLayer::KeywordScan,
                    category: sp.category.clone(),
                    matched_text: mat.as_str().to_string(),
                    offset: mat.start(),
                    length: mat.len(),
                    reason: sp.description.to_string(),
                });
            }
        }
    }

    deduplicate_violations(&mut violations);

    // N.2: Filter out false positives using exception patterns
    // Check both original and stripped text for exception patterns
    violations.retain(|v| !is_exception(text, v.offset) && !is_exception(&stripped, v.offset));

    violations
}

/// Remove overlapping violations, keeping the more specific (longer) match.
pub fn deduplicate_violations(violations: &mut Vec<Violation>) {
    violations.sort_by_key(|v| (v.offset, std::cmp::Reverse(v.length)));
    let mut i = 0;
    while i < violations.len() {
        let mut j = i + 1;
        while j < violations.len() {
            let vi_end = violations[i].offset + violations[i].length;
            let vj_end = violations[j].offset + violations[j].length;
            // If vj is fully contained within vi, remove vj
            if violations[j].offset >= violations[i].offset && vj_end <= vi_end {
                violations.remove(j);
            } else {
                j += 1;
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =================================================================
    // N.4: LANGUAGE SUPPORT
    // =================================================================

    #[test]
    fn supported_languages_include_en_fr_de() {
        assert!(is_supported_language("en"));
        assert!(is_supported_language("fr"));
        assert!(is_supported_language("de"));
    }

    #[test]
    fn unsupported_language_rejected() {
        assert!(!is_supported_language("zh"));
        assert!(!is_supported_language("ja"));
        assert!(!is_supported_language(""));
    }

    // =================================================================
    // N.3: ACCENT STRIPPING
    // =================================================================

    #[test]
    fn strip_accents_french() {
        assert_eq!(strip_accents("été"), "ete");
        assert_eq!(strip_accents("diagnostiqué"), "diagnostique");
        assert_eq!(strip_accents("médecin"), "medecin");
        assert_eq!(strip_accents("hôpital"), "hopital");
        assert_eq!(strip_accents("ça"), "ca");
    }

    #[test]
    fn strip_accents_preserves_ascii() {
        assert_eq!(strip_accents("hello world"), "hello world");
        assert_eq!(strip_accents("test123"), "test123");
    }

    #[test]
    fn strip_accents_german() {
        assert_eq!(strip_accents("über"), "uber");
        assert_eq!(strip_accents("Ärzt"), "Arzt");
        assert_eq!(strip_accents("Straße"), "Strasse");
    }

    // =================================================================
    // LAYER 2: KEYWORD SCAN — ENGLISH DIAGNOSTIC
    // =================================================================

    #[test]
    fn keyword_you_have_diabetes() {
        let violations = scan_keywords("Based on the symptoms, you have diabetes.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn keyword_you_are_suffering_from() {
        let violations = scan_keywords("You are suffering from chronic pain.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn keyword_you_likely_have() {
        let violations = scan_keywords("You likely have an infection.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn keyword_you_are_diabetic() {
        let violations = scan_keywords("Since you're diabetic, watch your sugar.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn keyword_your_condition_is() {
        let violations = scan_keywords("Your condition is worsening over time.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn keyword_you_appear_to_have() {
        let violations = scan_keywords("You appear to have a thyroid condition.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    // =================================================================
    // LAYER 2: KEYWORD SCAN — ENGLISH PRESCRIPTIVE
    // =================================================================

    #[test]
    fn keyword_you_should_take() {
        let violations = scan_keywords("You should take aspirin daily.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn keyword_you_should_stop() {
        let violations = scan_keywords("You should stop taking ibuprofen.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn keyword_i_recommend() {
        let violations = scan_keywords("I recommend starting a low-sodium diet.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn keyword_you_need_to_see() {
        let violations = scan_keywords("You need to see a specialist immediately.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn keyword_do_not_take() {
        let violations = scan_keywords("Do not take this medication with alcohol.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn keyword_try_taking() {
        let violations = scan_keywords("Try taking this supplement in the morning.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn keyword_consider_stopping() {
        let violations = scan_keywords("Consider stopping the medication before surgery.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    // =================================================================
    // LAYER 2: KEYWORD SCAN — ENGLISH ALARM
    // =================================================================

    #[test]
    fn keyword_dangerous() {
        let violations = scan_keywords("This interaction could be dangerous.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn keyword_immediately_go() {
        let violations = scan_keywords("Immediately go to the emergency room.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn keyword_call_911() {
        let violations = scan_keywords("Call 911 right away.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn keyword_seek_immediate_medical_attention() {
        let violations = scan_keywords("Seek immediate medical attention.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn keyword_life_threatening() {
        let violations = scan_keywords("This could be life-threatening.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn keyword_this_is_emergency() {
        let violations = scan_keywords("This is a medical emergency.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn keyword_do_not_wait() {
        let violations = scan_keywords("Do not wait to seek treatment.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    // =================================================================
    // N.1: FRENCH DIAGNOSTIC
    // =================================================================

    #[test]
    fn fr_vous_avez_diabetes() {
        let violations = scan_keywords("Vous avez un diabète de type 2.");
        assert!(!violations.is_empty(), "Should detect French diagnostic");
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn fr_vous_souffrez_de() {
        let violations = scan_keywords("Vous souffrez de douleurs chroniques.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn fr_vous_etes_probablement() {
        let violations = scan_keywords("Vous êtes probablement diabétique.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn fr_cela_signifie_que_vous() {
        let violations = scan_keywords("Cela signifie que vous avez une infection.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn fr_vous_avez_ete_diagnostique() {
        let violations = scan_keywords("Vous avez été diagnostiqué avec le diabète.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn fr_vous_etes_diabetique() {
        let violations = scan_keywords("Vous êtes diabétique.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn fr_votre_etat_est() {
        let violations = scan_keywords("Votre état est préoccupant.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn fr_vous_semblez_avoir() {
        let violations = scan_keywords("Vous semblez avoir une infection.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    // =================================================================
    // N.1: FRENCH PRESCRIPTIVE
    // =================================================================

    #[test]
    fn fr_vous_devriez_prendre() {
        let violations = scan_keywords("Vous devriez prendre de l'insuline.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn fr_je_recommande() {
        let violations = scan_keywords("Je vous recommande ce traitement.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn fr_je_suggere() {
        let violations = scan_keywords("Je suggère de réduire la dose.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn fr_vous_devez_prendre() {
        let violations = scan_keywords("Vous devez prendre ce médicament.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn fr_ne_prenez_pas() {
        let violations = scan_keywords("Ne prenez pas ce médicament avec de l'alcool.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn fr_essayez_de_prendre() {
        let violations = scan_keywords("Essayez de prendre ce supplément le matin.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn fr_il_faut_consulter() {
        let violations = scan_keywords("Il faut consulter un spécialiste.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    // =================================================================
    // N.1: FRENCH ALARM
    // =================================================================

    #[test]
    fn fr_dangereux() {
        let violations = scan_keywords("Cette interaction est dangereuse.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn fr_urgence() {
        let violations = scan_keywords("C'est une urgence médicale.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn fr_appelez_le_15() {
        let violations = scan_keywords("Appelez le 15 immédiatement.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn fr_allez_aux_urgences() {
        let violations = scan_keywords("Allez aux urgences tout de suite.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn fr_n_attendez_pas() {
        let violations = scan_keywords("N'attendez pas pour consulter.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn fr_mortel() {
        let violations = scan_keywords("Cette dose pourrait être mortelle.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    // =================================================================
    // N.3: ACCENT-INSENSITIVE MATCHING
    // =================================================================

    #[test]
    fn accent_insensitive_french_diagnostic() {
        // Both NFC (precomposed é) and decomposed should match
        let violations = scan_keywords("Vous avez été diagnostiqué avec le diabète.");
        assert!(!violations.is_empty(), "Accented French should be detected");
    }

    #[test]
    fn accent_insensitive_french_alarm() {
        let violations = scan_keywords("C'est une urgence médicale, allez à l'hôpital.");
        assert!(!violations.is_empty(), "Accented French alarm should be detected");
    }

    // =================================================================
    // N.2: FALSE POSITIVE EXCEPTIONS
    // =================================================================

    #[test]
    fn exception_your_documents_show() {
        let text = "Your documents show that you have been prescribed metformin.";
        let violations = scan_keywords(text);
        assert!(violations.is_empty(), "Grounded text should be exempted, got: {:?}", violations);
    }

    #[test]
    fn exception_according_to_records() {
        let text = "According to your records, your condition is being monitored by Dr. Chen.";
        let violations = scan_keywords(text);
        assert!(violations.is_empty(), "Document-attributed text should be exempted, got: {:?}", violations);
    }

    #[test]
    fn exception_doctor_noted() {
        let text = "Your doctor noted that you have diabetes and prescribed metformin.";
        let violations = scan_keywords(text);
        assert!(violations.is_empty(), "Doctor-attributed text should be exempted, got: {:?}", violations);
    }

    #[test]
    fn exception_fr_selon_documents() {
        let text = "Selon vos documents, vous avez un diabète de type 2.";
        let violations = scan_keywords(text);
        assert!(violations.is_empty(), "French grounded text should be exempted, got: {:?}", violations);
    }

    #[test]
    fn no_exception_for_ungrounded_diagnostic() {
        // Without document attribution, violation should still be caught
        let text = "You have diabetes and should manage it carefully.";
        let violations = scan_keywords(text);
        assert!(!violations.is_empty(), "Ungrounded diagnostic should be caught");
    }

    // =================================================================
    // CLEAN PASS + EDGE CASES
    // =================================================================

    #[test]
    fn clean_text_no_violations() {
        let violations = scan_keywords(
            "Your documents show that Dr. Chen prescribed metformin 500mg twice daily. \
             This was documented on January 15, 2024.",
        );
        assert!(violations.is_empty(), "Expected no violations, got: {:?}", violations);
    }

    #[test]
    fn case_insensitive_detection() {
        let violations_lower = scan_keywords("you should take aspirin.");
        let violations_upper = scan_keywords("You Should Take aspirin.");
        let violations_mixed = scan_keywords("YOU SHOULD TAKE aspirin.");
        assert!(!violations_lower.is_empty());
        assert!(!violations_upper.is_empty());
        assert!(!violations_mixed.is_empty());
    }

    #[test]
    fn deduplicate_overlapping() {
        let mut violations = vec![
            Violation {
                layer: FilterLayer::KeywordScan,
                category: ViolationCategory::AlarmLanguage,
                matched_text: "immediately go to the emergency".to_string(),
                offset: 0,
                length: 32,
                reason: "test".to_string(),
            },
            Violation {
                layer: FilterLayer::KeywordScan,
                category: ViolationCategory::AlarmLanguage,
                matched_text: "immediately go".to_string(),
                offset: 0,
                length: 14,
                reason: "test".to_string(),
            },
        ];
        deduplicate_violations(&mut violations);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].length, 32);
    }

    #[test]
    fn safe_document_attribution_passes() {
        let text = "According to your documents, your doctor noted a diagnosis of hypertension. \
                     You might want to ask your doctor about any changes to your treatment plan.";
        let violations = scan_keywords(text);
        assert!(violations.is_empty(), "Got violations: {:?}", violations);
    }

    // =================================================================
    // ADVERSARIAL / RED-TEAM TESTS (RS-L2-02-001)
    // =================================================================

    #[test]
    fn adversarial_diagnostic_with_hedge_words() {
        let texts = [
            "You likely have an infection based on these results.",
            "You probably have type 2 diabetes.",
            "You possibly have a thyroid condition.",
            "You appear to have cardiovascular disease.",
            "You seem to have a chronic condition.",
        ];
        for text in texts {
            let violations = scan_keywords(text);
            assert!(!violations.is_empty(), "Should catch hedged diagnosis: {text}");
        }
    }

    #[test]
    fn adversarial_prescriptive_with_soft_framing() {
        let texts = [
            "Consider taking this supplement daily.",
            "Try taking the medication in the morning.",
            "The best treatment would be antibiotics.",
            "The recommended course of action is surgery.",
        ];
        for text in texts {
            let violations = scan_keywords(text);
            assert!(!violations.is_empty(), "Should catch soft prescription: {text}");
        }
    }

    #[test]
    fn adversarial_alarm_with_medical_terminology() {
        let texts = [
            "This could be life threatening if left untreated.",
            "This is a medical emergency requiring immediate attention.",
            "Seek urgent medical care for this condition.",
            "Go to the ER as soon as possible.",
            "Do not delay treatment any further.",
        ];
        for text in texts {
            let violations = scan_keywords(text);
            assert!(!violations.is_empty(), "Should catch clinical alarm: {text}");
        }
    }

    #[test]
    fn adversarial_multi_violation_response() {
        let text = "You have diabetes. You should take metformin. \
                    This is dangerous and you should immediately go to the hospital.";
        let violations = scan_keywords(text);
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage),
            "Should detect diagnostic violation"
        );
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage),
            "Should detect prescriptive violation"
        );
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage),
            "Should detect alarm violation"
        );
    }

    #[test]
    fn adversarial_embedded_in_long_safe_text() {
        let text = "Your documents from January 2024 show several lab results. \
                    Dr. Chen documented your hemoglobin at 13.5 g/dL, which is within \
                    the normal reference range of 12.0-17.5 g/dL. Your cholesterol panel \
                    showed total cholesterol at 210 mg/dL. You should stop taking the \
                    supplement before your next test. The creatinine was 1.1 mg/dL.";
        let violations = scan_keywords(text);
        assert!(!violations.is_empty(), "Should catch embedded violation");
        assert!(violations
            .iter()
            .any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn adversarial_safe_responses_not_flagged() {
        let safe_texts = [
            "Your documents show that Dr. Chen prescribed metformin 500mg twice daily.",
            "According to your records, your last A1c was 7.2%.",
            "Your lab results from January indicate a hemoglobin of 13.5 g/dL.",
            "You might want to ask your healthcare provider about this result.",
            "This is something you may want to discuss with your doctor.",
        ];
        for text in safe_texts {
            let violations = scan_keywords(text);
            assert!(violations.is_empty(), "False positive on safe text: {text}");
        }
    }

    // =================================================================
    // N.1: FRENCH ADVERSARIAL / MULTI-LANGUAGE
    // =================================================================

    #[test]
    fn fr_adversarial_multi_violation() {
        let text = "Vous avez un diabète. Vous devriez prendre de l'insuline. \
                    C'est dangereux et vous devez immédiatement aller aux urgences.";
        let violations = scan_keywords(text);
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage),
            "Should detect French diagnostic"
        );
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage),
            "Should detect French prescriptive"
        );
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage),
            "Should detect French alarm"
        );
    }

    #[test]
    fn mixed_language_both_detected() {
        // English diagnostic + French alarm in same response
        let text = "You have diabetes. Allez aux urgences immédiatement.";
        let violations = scan_keywords(text);
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage),
            "Should detect English diagnostic"
        );
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage),
            "Should detect French alarm"
        );
    }

    // =================================================================
    // I18N-05: GERMAN DIAGNOSTIC
    // =================================================================

    #[test]
    fn de_sie_haben_diabetes() {
        let violations = scan_keywords("Sie haben einen Diabetes Typ 2.");
        assert!(!violations.is_empty(), "Should detect German diagnostic");
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn de_sie_leiden_an() {
        let violations = scan_keywords("Sie leiden an chronischen Schmerzen.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn de_sie_sind_wahrscheinlich() {
        let violations = scan_keywords("Sie sind wahrscheinlich zuckerkrank.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn de_das_bedeutet_dass_sie() {
        let violations = scan_keywords("Das bedeutet dass Sie eine Infektion haben.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn de_bei_ihnen_diagnostiziert() {
        let violations = scan_keywords("Bei Ihnen wurde Diabetes diagnostiziert.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn de_sie_sind_diabetiker() {
        let violations = scan_keywords("Sie sind Diabetiker und sollten aufpassen.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn de_ihr_zustand_ist() {
        let violations = scan_keywords("Ihr Zustand ist besorgniserregend.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn de_sie_scheinen_zu_haben() {
        let violations = scan_keywords("Sie scheinen eine Infektion zu haben.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    // =================================================================
    // I18N-05: GERMAN PRESCRIPTIVE
    // =================================================================

    #[test]
    fn de_sie_sollten_einnehmen() {
        let violations = scan_keywords("Sie sollten dieses Medikament einnehmen.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn de_ich_empfehle() {
        let violations = scan_keywords("Ich empfehle diese Behandlung.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn de_sie_mussen_einnehmen() {
        let violations = scan_keywords("Sie müssen dieses Medikament einnehmen.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn de_nehmen_sie_nicht() {
        let violations = scan_keywords("Nehmen Sie nicht dieses Medikament mit Alkohol.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn de_die_beste_behandlung() {
        let violations = scan_keywords("Die beste Behandlung wäre eine Operation.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    // =================================================================
    // I18N-05: GERMAN ALARM
    // =================================================================

    #[test]
    fn de_gefahrlich() {
        let violations = scan_keywords("Diese Wechselwirkung ist gefährlich.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn de_notfall() {
        let violations = scan_keywords("Dies ist ein medizinischer Notfall.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn de_rufen_sie_112() {
        let violations = scan_keywords("Rufen Sie die 112 sofort an.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn de_gehen_sie_notaufnahme() {
        let violations = scan_keywords("Gehen Sie in die Notaufnahme.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn de_lebensbedrohlich() {
        let violations = scan_keywords("Dieser Zustand ist lebensbedrohlich.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn de_warten_sie_nicht() {
        let violations = scan_keywords("Warten Sie nicht mit der Behandlung.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    // =================================================================
    // I18N-05: GERMAN ACCENT-INSENSITIVE + EXCEPTIONS
    // =================================================================

    #[test]
    fn de_accent_insensitive_umlaut() {
        // ü→u, ö→o, ä→a should still match
        let violations = scan_keywords("Sie müssen dieses Medikament einnehmen.");
        assert!(!violations.is_empty(), "Umlauted German should be detected");
    }

    #[test]
    fn de_accent_insensitive_eszett() {
        // ß→ss in strip_accents
        let violations = scan_keywords("Die beste Maßnahme wäre sofort zum Arzt.");
        assert!(!violations.is_empty(), "ß-containing German should be detected");
    }

    #[test]
    fn de_exception_laut_unterlagen() {
        let text = "Laut Ihren Unterlagen haben Sie einen Diabetes Typ 2.";
        let violations = scan_keywords(text);
        assert!(violations.is_empty(), "German grounded text should be exempted, got: {:?}", violations);
    }

    #[test]
    fn de_exception_ihr_arzt_hat() {
        let text = "Ihr Arzt hat festgestellt dass Sie Diabetes haben.";
        let violations = scan_keywords(text);
        assert!(violations.is_empty(), "German doctor-attributed text should be exempted, got: {:?}", violations);
    }

    #[test]
    fn de_no_exception_ungrounded() {
        let text = "Sie haben Diabetes und sollten aufpassen.";
        let violations = scan_keywords(text);
        assert!(!violations.is_empty(), "Ungrounded German diagnostic should be caught");
    }

    // =================================================================
    // I18N-05: GERMAN ADVERSARIAL / MULTI-LANGUAGE
    // =================================================================

    #[test]
    fn de_adversarial_multi_violation() {
        let text = "Sie haben Diabetes. Sie sollten dieses Medikament einnehmen. \
                    Das ist gefährlich und Sie müssen sofort ins Krankenhaus.";
        let violations = scan_keywords(text);
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage),
            "Should detect German diagnostic"
        );
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage),
            "Should detect German prescriptive"
        );
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage),
            "Should detect German alarm"
        );
    }

    #[test]
    fn mixed_language_en_de_detected() {
        // English diagnostic + German alarm in same response
        let text = "You have diabetes. Gehen Sie sofort in die Notaufnahme.";
        let violations = scan_keywords(text);
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage),
            "Should detect English diagnostic"
        );
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage),
            "Should detect German alarm"
        );
    }

    #[test]
    fn mixed_language_all_three_detected() {
        let text = "You have diabetes. Vous souffrez de douleurs. Dies ist ein Notfall.";
        let violations = scan_keywords(text);
        // Should detect violations from all three languages
        assert!(violations.len() >= 3, "Should detect from all 3 languages, got: {:?}", violations);
    }
}
