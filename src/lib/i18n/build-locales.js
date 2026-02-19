/**
 * SE-002: Build script for i18n locale module assembly.
 *
 * Merges domain-scoped JSON modules (locales/modules/{lang}/*.json)
 * into single locale files (locales/_generated/{lang}.json).
 *
 * Validates:
 *   1. Key completeness — all EN keys must exist in FR and DE
 *   2. No orphaned keys — keys in FR/DE not in EN are flagged
 *   3. ICU message syntax — balanced braces in format strings
 *   4. Placeholder consistency — {name} patterns match across locales
 *
 * Missing keys or validation failures cause a build failure.
 *
 * Usage:
 *   node src/lib/i18n/build-locales.js          # Build
 *   node src/lib/i18n/build-locales.js --check   # Validate only (no write)
 */

import { readdirSync, readFileSync, writeFileSync, mkdirSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const LANGUAGES = ['en', 'fr', 'de'];
const MODULES_DIR = join(__dirname, 'locales/modules');
const OUTPUT_DIR = join(__dirname, 'locales/_generated');
const CHECK_ONLY = process.argv.includes('--check');

function mergeModules(lang) {
  const langDir = join(MODULES_DIR, lang);

  if (!existsSync(langDir)) {
    console.error(`Missing module directory: ${langDir}`);
    process.exit(1);
  }

  const files = readdirSync(langDir)
    .filter(f => f.endsWith('.json'))
    .sort();

  const merged = {};

  for (const file of files) {
    const content = JSON.parse(readFileSync(join(langDir, file), 'utf-8'));
    Object.assign(merged, content);
  }

  return merged;
}

function validateCompleteness(reference, target, lang) {
  const errors = [];

  function checkKeys(ref, tgt, path) {
    for (const key of Object.keys(ref)) {
      const fullPath = path ? `${path}.${key}` : key;
      if (!(key in tgt)) {
        errors.push(`MISSING in ${lang}: ${fullPath}`);
      } else if (typeof ref[key] === 'object' && ref[key] !== null) {
        checkKeys(
          ref[key],
          tgt[key] ?? {},
          fullPath
        );
      }
    }
  }

  checkKeys(reference, target, '');
  return errors;
}

function validateOrphanedKeys(reference, target, lang) {
  const errors = [];

  function checkKeys(ref, tgt, path) {
    for (const key of Object.keys(tgt)) {
      const fullPath = path ? `${path}.${key}` : key;
      if (!(key in ref)) {
        errors.push(`ORPHAN in ${lang}: ${fullPath} (not in EN)`);
      } else if (typeof tgt[key] === 'object' && tgt[key] !== null) {
        checkKeys(ref[key] ?? {}, tgt[key], fullPath);
      }
    }
  }

  checkKeys(reference, target, '');
  return errors;
}

function extractPlaceholders(value) {
  // Extract only top-level ICU placeholders (depth 0 → depth 1 transitions).
  // Ignores nested content in plural/select branches.
  const names = new Set();
  let depth = 0;
  let i = 0;
  while (i < value.length) {
    if (value[i] === '{') {
      depth++;
      if (depth === 1) {
        // Capture the placeholder name (first word after {)
        const rest = value.slice(i + 1);
        const match = rest.match(/^(\w+)/);
        if (match) names.add(match[1]);
      }
      i++;
    } else if (value[i] === '}') {
      depth--;
      i++;
    } else {
      i++;
    }
  }
  return Array.from(names).sort();
}

function validateIcuSyntax(obj, lang) {
  const errors = [];

  function check(value, path) {
    if (typeof value === 'object' && value !== null) {
      for (const [k, v] of Object.entries(value)) {
        check(v, path ? `${path}.${k}` : k);
      }
      return;
    }
    if (typeof value !== 'string') return;

    let depth = 0;
    for (let i = 0; i < value.length; i++) {
      if (value[i] === '{') depth++;
      else if (value[i] === '}') depth--;
      if (depth < 0) {
        errors.push(`ICU SYNTAX in ${lang}: ${path} — unmatched closing brace at position ${i}`);
        return;
      }
    }
    if (depth !== 0) {
      errors.push(`ICU SYNTAX in ${lang}: ${path} — ${depth} unclosed brace(s)`);
    }
  }

  check(obj, '');
  return errors;
}

function validatePlaceholderConsistency(reference, target, lang) {
  const errors = [];

  function check(ref, tgt, path) {
    for (const key of Object.keys(ref)) {
      const fullPath = path ? `${path}.${key}` : key;
      if (!(key in tgt)) continue; // Missing keys caught by completeness check

      if (typeof ref[key] === 'object' && ref[key] !== null) {
        check(ref[key], tgt[key] ?? {}, fullPath);
      } else if (typeof ref[key] === 'string' && typeof tgt[key] === 'string') {
        const enPlaceholders = extractPlaceholders(ref[key]);
        const tgtPlaceholders = extractPlaceholders(tgt[key]);

        if (enPlaceholders.join(',') !== tgtPlaceholders.join(',')) {
          errors.push(
            `PLACEHOLDER in ${lang}: ${fullPath} — EN has {${enPlaceholders.join(', ')}}, ${lang} has {${tgtPlaceholders.join(', ')}}`
          );
        }
      }
    }
  }

  check(reference, target, '');
  return errors;
}

function validateModuleConsistency() {
  const enFiles = readdirSync(join(MODULES_DIR, 'en'))
    .filter(f => f.endsWith('.json'))
    .sort();

  const errors = [];

  for (const lang of LANGUAGES.filter(l => l !== 'en')) {
    const langDir = join(MODULES_DIR, lang);
    if (!existsSync(langDir)) {
      errors.push(`Missing module directory for ${lang}`);
      continue;
    }

    const langFiles = readdirSync(langDir)
      .filter(f => f.endsWith('.json'))
      .sort();

    for (const file of enFiles) {
      if (!langFiles.includes(file)) {
        errors.push(`Missing module file: ${lang}/${file}`);
      }
    }

    for (const file of langFiles) {
      if (!enFiles.includes(file)) {
        errors.push(`Extra module file (no EN counterpart): ${lang}/${file}`);
      }
    }
  }

  return errors;
}

// Build
const structureErrors = validateModuleConsistency();
if (structureErrors.length > 0) {
  console.error('\nModule structure errors:');
  structureErrors.forEach(e => console.error(`  ${e}`));
  process.exit(1);
}

const enMerged = mergeModules('en');
let hasErrors = false;
let warningCount = 0;

// ICU syntax check on all languages (including EN)
for (const lang of LANGUAGES) {
  const merged = lang === 'en' ? enMerged : mergeModules(lang);
  const icuErrors = validateIcuSyntax(merged, lang);
  if (icuErrors.length > 0) {
    console.error(`\n${lang}: ${icuErrors.length} ICU syntax error(s):`);
    icuErrors.forEach(e => console.error(`  ${e}`));
    hasErrors = true;
  }
}

// Completeness, orphan, and placeholder checks on non-EN languages
for (const lang of LANGUAGES.filter(l => l !== 'en')) {
  const merged = mergeModules(lang);

  const missingErrors = validateCompleteness(enMerged, merged, lang);
  if (missingErrors.length > 0) {
    console.error(`\n${lang}: ${missingErrors.length} missing key(s):`);
    missingErrors.forEach(e => console.error(`  ${e}`));
    hasErrors = true;
  }

  const orphanErrors = validateOrphanedKeys(enMerged, merged, lang);
  if (orphanErrors.length > 0) {
    console.warn(`\n${lang}: ${orphanErrors.length} orphaned key(s):`);
    orphanErrors.forEach(e => console.warn(`  ${e}`));
    warningCount += orphanErrors.length;
  }

  const placeholderErrors = validatePlaceholderConsistency(enMerged, merged, lang);
  if (placeholderErrors.length > 0) {
    console.error(`\n${lang}: ${placeholderErrors.length} placeholder mismatch(es):`);
    placeholderErrors.forEach(e => console.error(`  ${e}`));
    hasErrors = true;
  }
}

if (hasErrors) {
  process.exit(1);
}

if (CHECK_ONLY) {
  const msg = `Validation passed: ${LANGUAGES.length} languages, all keys present.`;
  console.log(warningCount > 0 ? `${msg} (${warningCount} warnings)` : msg);
  process.exit(0);
}

mkdirSync(OUTPUT_DIR, { recursive: true });

for (const lang of LANGUAGES) {
  const merged = mergeModules(lang);
  writeFileSync(
    join(OUTPUT_DIR, `${lang}.json`),
    JSON.stringify(merged, null, 2) + '\n'
  );
}

console.log(`Built ${LANGUAGES.length} locale files from ${readdirSync(join(MODULES_DIR, 'en')).filter(f => f.endsWith('.json')).length} modules each.`);
