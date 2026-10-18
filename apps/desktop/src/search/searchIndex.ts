import type {SearchFixtureEntry} from '../app/fixtures.ts';

export interface NormalizedText {
  value: string;
  map: Array<{start: number; end: number}>;
}

export interface SearchResultView {
  id: string;
  entry: SearchFixtureEntry;
  matchRange: {start: number; end: number};
  normalizedRange: {start: number; end: number};
  alternateAnchors: string[];
}

interface GraphemeSegment {
  segment: string;
  index: number;
}

function graphemes(text: string): GraphemeSegment[] {
  const segmenterType = (Intl as unknown as {
    Segmenter?: new (locales?: string[], options?: {granularity: 'grapheme'}) => {segment(input: string): Iterable<{segment: string; index: number}>};
  }).Segmenter;
  if (segmenterType) {
    const segmenter = new segmenterType(['zh', 'en'], {granularity: 'grapheme'});
    return Array.from(segmenter.segment(text));
  }

  return Array.from(text).map((segment, index) => ({segment, index}));
}

function caseFold(value: string): string {
  // JS lower-case is close to Unicode casefold for the UI profile; cover the
  // fixture's ß expansion explicitly so its stable anchor is testable.
  return value.toLocaleLowerCase('und').replaceAll('ß', 'ss').replaceAll('ẞ', 'ss');
}

function isWhitespace(value: string): boolean {
  return /^\s+$/u.test(value);
}

export function normalizeWithMap(text: string, caseSensitive = false): NormalizedText {
  let value = '';
  const map: Array<{start: number; end: number}> = [];
  let scalarOffset = 0;
  let pendingWhitespace: {start: number; end: number} | null = null;

  for (const {segment} of graphemes(text)) {
    const segmentStart = scalarOffset;
    scalarOffset += Array.from(segment).length;
    const segmentEnd = scalarOffset;

    if (isWhitespace(segment)) {
      if (value.length > 0 && !value.endsWith(' ')) {
        value += ' ';
        map.push({start: segmentStart, end: segmentEnd});
      } else if (value.endsWith(' ') && map.length > 0) {
        map[map.length - 1] = {start: map[map.length - 1].start, end: segmentEnd};
      } else {
        pendingWhitespace = {start: segmentStart, end: segmentEnd};
      }
      continue;
    }

    const normalizedSegment = (caseSensitive ? segment : caseFold(segment)).normalize('NFC');
    if (pendingWhitespace && value.length > 0 && !value.endsWith(' ')) {
      value += ' ';
      map.push(pendingWhitespace);
    }
    pendingWhitespace = null;
    for (const character of Array.from(normalizedSegment)) {
      value += character;
      map.push({start: segmentStart, end: segmentEnd});
    }
  }

  return {value, map};
}

export function searchFixtureEntries(
  entries: SearchFixtureEntry[],
  query: string,
  kind: SearchFixtureEntry['kind'] | 'all' = 'all',
  caseSensitive = false,
): SearchResultView[] {
  const normalizedQuery = normalizeWithMap(query, caseSensitive).value.trim();
  if (!normalizedQuery) return [];
  const results: SearchResultView[] = [];

  for (const entry of entries) {
    if (kind !== 'all' && entry.kind !== kind) continue;
    const normalized = normalizeWithMap(entry.text, caseSensitive);
    let fromIndex = 0;
    while (fromIndex <= normalized.value.length - normalizedQuery.length) {
      const index = normalized.value.indexOf(normalizedQuery, fromIndex);
      if (index === -1) break;
      const end = index + normalizedQuery.length;
      const first = normalized.map[index];
      const last = normalized.map[end - 1];
      if (first && last) {
        results.push({
          id: `${entry.id}:${index}`,
          entry,
          matchRange: {start: first.start, end: last.end},
          normalizedRange: {start: index, end},
          alternateAnchors: [],
        });
      }
      fromIndex = index + Math.max(1, normalizedQuery.length);
    }
  }

  const byEquivalence = new Map<string, SearchResultView>();
  const merged: SearchResultView[] = [];
  for (const result of results) {
    const groupKey = result.entry.equivalenceGroupId ? `${result.entry.equivalenceGroupId}:${result.normalizedRange.start}` : result.id;
    const existing = byEquivalence.get(groupKey);
    if (existing) {
      existing.alternateAnchors.push(result.entry.anchorLabel);
    } else {
      byEquivalence.set(groupKey, result);
      merged.push(result);
    }
  }
  return merged;
}

export function snippet(text: string, range: {start: number; end: number}, radius = 32): string {
  const scalars = Array.from(text);
  const start = Math.max(0, range.start - radius);
  const end = Math.min(scalars.length, range.end + radius);
  return `${start > 0 ? '…' : ''}${scalars.slice(start, end).join('')}${end < scalars.length ? '…' : ''}`;
}
