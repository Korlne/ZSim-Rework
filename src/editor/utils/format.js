/**
 * Number formatting utilities for data entry display.
 */

/**
 * Format a number with thousands separators.
 */
export function formatNumber(value) {
  if (value === null || value === undefined) return "-";
  return Number(value).toLocaleString();
}

/**
 * Format a decimal as a percentage string (0.12 -> "12%").
 */
export function formatPercent(value) {
  if (value === null || value === undefined) return "-";
  return `${(Number(value) * 100).toFixed(1)}%`;
}

/**
 * Format a number to a fixed number of decimal places.
 */
export function formatFixed(value, decimals = 2) {
  if (value === null || value === undefined) return "-";
  return Number(value).toFixed(decimals);
}

/**
 * Format a float value for stat display.
 * Large numbers use fewer decimals, small values show more precision.
 */
export function formatStat(value) {
  if (value === null || value === undefined) return "-";
  const num = Number(value);
  if (num === 0) return "0";
  if (Math.abs(num) >= 1000) return num.toLocaleString(undefined, { maximumFractionDigits: 0 });
  if (Math.abs(num) >= 1) return num.toLocaleString(undefined, { maximumFractionDigits: 2 });
  if (Math.abs(num) >= 0.01) return num.toFixed(3);
  return num.toExponential(2);
}

/**
 * Format an energy/regen value (shows 2 decimals).
 */
export function formatEnergy(value) {
  if (value === null || value === undefined) return "-";
  return Number(value).toFixed(2);
}

/**
 * Truncate a long string with ellipsis.
 */
export function truncate(value, maxLen = 30) {
  if (!value) return "-";
  const str = String(value);
  if (str.length <= maxLen) return str;
  return str.slice(0, maxLen - 3) + "...";
}

/**
 * Format a list of strings as a comma-separated display string.
 */
export function formatList(items) {
  if (!items || items.length === 0) return "-";
  return items.join(", ");
}

/**
 * Format object as JSON string for display in a <pre> context.
 */
export function formatJson(value) {
  if (value === null || value === undefined) return "-";
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}
