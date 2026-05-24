/**
 * Country list for the bank-account location picker (Feroz step C / §12).
 *
 * Feroz was explicit on the May-17 call: a bank account must capture a
 * country, but the country must NOT auto-map to a currency — "I can
 * have an account in Singapore with USD". So this list is purely for
 * labeling where the bank is; currency is chosen independently.
 *
 * `code` is ISO 3166-1 alpha-2 (used only to derive the flag emoji).
 * `name` is what we persist in the asset metadata and show to the user,
 * so it stays human-readable without a code→name resolver downstream.
 */

export interface Country {
  code: string;
  name: string;
}

/**
 * Comprehensive list covering every region. Ordered roughly by global
 * relevance to a multi-national investor (Feroz's own examples — Saudi
 * Arabia, UAE, Singapore, India, Pakistan — are all present), then
 * alphabetical for the long tail.
 */
export const COUNTRIES: readonly Country[] = [
  { code: "US", name: "United States" },
  { code: "GB", name: "United Kingdom" },
  { code: "SG", name: "Singapore" },
  { code: "AE", name: "United Arab Emirates" },
  { code: "SA", name: "Saudi Arabia" },
  { code: "IN", name: "India" },
  { code: "PK", name: "Pakistan" },
  { code: "CA", name: "Canada" },
  { code: "AU", name: "Australia" },
  { code: "HK", name: "Hong Kong" },
  { code: "CN", name: "China" },
  { code: "JP", name: "Japan" },
  { code: "DE", name: "Germany" },
  { code: "FR", name: "France" },
  { code: "CH", name: "Switzerland" },
  { code: "QA", name: "Qatar" },
  { code: "KW", name: "Kuwait" },
  { code: "BH", name: "Bahrain" },
  { code: "OM", name: "Oman" },
  { code: "MY", name: "Malaysia" },
  { code: "ID", name: "Indonesia" },
  { code: "BD", name: "Bangladesh" },
  { code: "LK", name: "Sri Lanka" },
  { code: "AF", name: "Afghanistan" },
  { code: "AL", name: "Albania" },
  { code: "DZ", name: "Algeria" },
  { code: "AR", name: "Argentina" },
  { code: "AT", name: "Austria" },
  { code: "BE", name: "Belgium" },
  { code: "BR", name: "Brazil" },
  { code: "BN", name: "Brunei" },
  { code: "BG", name: "Bulgaria" },
  { code: "KH", name: "Cambodia" },
  { code: "CL", name: "Chile" },
  { code: "CO", name: "Colombia" },
  { code: "HR", name: "Croatia" },
  { code: "CY", name: "Cyprus" },
  { code: "CZ", name: "Czechia" },
  { code: "DK", name: "Denmark" },
  { code: "EG", name: "Egypt" },
  { code: "EE", name: "Estonia" },
  { code: "FI", name: "Finland" },
  { code: "GE", name: "Georgia" },
  { code: "GH", name: "Ghana" },
  { code: "GR", name: "Greece" },
  { code: "HU", name: "Hungary" },
  { code: "IS", name: "Iceland" },
  { code: "IE", name: "Ireland" },
  { code: "IL", name: "Israel" },
  { code: "IT", name: "Italy" },
  { code: "JO", name: "Jordan" },
  { code: "KZ", name: "Kazakhstan" },
  { code: "KE", name: "Kenya" },
  { code: "KR", name: "South Korea" },
  { code: "LB", name: "Lebanon" },
  { code: "LU", name: "Luxembourg" },
  { code: "MV", name: "Maldives" },
  { code: "MT", name: "Malta" },
  { code: "MU", name: "Mauritius" },
  { code: "MX", name: "Mexico" },
  { code: "MA", name: "Morocco" },
  { code: "NP", name: "Nepal" },
  { code: "NL", name: "Netherlands" },
  { code: "NZ", name: "New Zealand" },
  { code: "NG", name: "Nigeria" },
  { code: "NO", name: "Norway" },
  { code: "PH", name: "Philippines" },
  { code: "PL", name: "Poland" },
  { code: "PT", name: "Portugal" },
  { code: "RO", name: "Romania" },
  { code: "RU", name: "Russia" },
  { code: "RW", name: "Rwanda" },
  { code: "ZA", name: "South Africa" },
  { code: "ES", name: "Spain" },
  { code: "SE", name: "Sweden" },
  { code: "TW", name: "Taiwan" },
  { code: "TZ", name: "Tanzania" },
  { code: "TH", name: "Thailand" },
  { code: "TR", name: "Türkiye" },
  { code: "UG", name: "Uganda" },
  { code: "UA", name: "Ukraine" },
  { code: "UY", name: "Uruguay" },
  { code: "UZ", name: "Uzbekistan" },
  { code: "VN", name: "Vietnam" },
  { code: "ZM", name: "Zambia" },
  { code: "ZW", name: "Zimbabwe" },
];

/**
 * Derive a flag emoji from an ISO 3166-1 alpha-2 code by mapping each
 * ASCII letter to its Regional Indicator Symbol. Pure, no asset files.
 * Returns "" for malformed codes so the UI just shows the name.
 */
export function countryFlag(code: string): string {
  if (!/^[A-Za-z]{2}$/.test(code)) return "";
  const base = 0x1f1e6;
  const upper = code.toUpperCase();
  return String.fromCodePoint(base + (upper.charCodeAt(0) - 65), base + (upper.charCodeAt(1) - 65));
}

/**
 * Picker options: value is the human-readable country name (what we
 * persist in metadata, so display needs no resolver), label prefixes
 * the flag for fast visual scanning.
 */
export const COUNTRY_SELECT_OPTIONS: readonly { value: string; label: string }[] = COUNTRIES.map(
  (c) => ({
    value: c.name,
    label: `${countryFlag(c.code)} ${c.name}`.trim(),
  }),
);
