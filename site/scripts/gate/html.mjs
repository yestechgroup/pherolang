/**
 * Check 3 — structural HTML validity over every built page in both apps.
 *
 * Astro emits its own opinionated markup and Starlight emits a lot of it, so
 * this runs over `dist/client/**\/*.html`, not over the sources: what ships is
 * what is measured. The ruleset and its two explicit exceptions are documented
 * in `html-validate.config.mjs`.
 *
 * This is deliberately not a formatter check and not an axe substitute.
 */

import { join } from 'node:path';
import { HtmlValidate } from 'html-validate';

import { configFor } from './html-validate.config.mjs';
import { APPS, SITE } from './paths.mjs';
import { plural, readText, walk } from './util.mjs';

const NAME = 'html';

/** How many findings to print before summarising the rest. */
const PRINT_LIMIT = 40;

export const check = {
  name: NAME,
  title: 'every built page validates structurally (html-validate:document)',

  async run() {
    const lines = [];
    const findings = [];
    let files = 0;

    for (const app of Object.values(APPS)) {
      const validator = new HtmlValidate(configFor(app.key));
      for (const rel of walk(app.dist, (file) => file.endsWith('.html'))) {
        const path = join(app.dist, rel);
        const report = await validator.validateString(readText(path), path);
        files += 1;
        for (const result of report.results) {
          for (const message of result.messages) {
            findings.push({
              file: `${app.key}:${rel}`,
              line: message.line,
              column: message.column,
              rule: message.ruleId,
              selector: message.selector,
              message: message.message,
            });
          }
        }
      }
    }

    const byRule = new Map();
    for (const finding of findings) byRule.set(finding.rule, (byRule.get(finding.rule) ?? 0) + 1);

    if (findings.length === 0) {
      lines.push(`ok   ${plural(files, 'built page')} validated with no findings`);
      return { status: 'pass', lines };
    }

    lines.push(
      `FAIL ${plural(findings.length, 'finding')} across ${plural(byRule.size, 'rule')} in ${plural(files, 'built page')}`,
    );
    lines.push(
      `     ${[...byRule].sort((a, b) => b[1] - a[1]).map(([rule, count]) => `${rule} x${count}`).join(', ')}`,
    );
    for (const finding of findings.slice(0, PRINT_LIMIT)) {
      const where = finding.selector ? ` [${finding.selector}]` : '';
      lines.push(`     ${finding.file}:${finding.line}:${finding.column} ${finding.rule}: ${finding.message}${where}`);
    }
    if (findings.length > PRINT_LIMIT) {
      lines.push(`     … and ${findings.length - PRINT_LIMIT} more`);
    }
    lines.push(`     config: ${join(SITE, 'scripts', 'gate', 'html-validate.config.mjs')}`);

    return { status: 'fail', lines };
  },
};
