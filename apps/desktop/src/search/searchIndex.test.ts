import {describe, expect, it} from 'vitest';
import {searchEntries} from '../app/fixtures.ts';
import {normalizeWithMap, searchFixtureEntries} from './searchIndex.ts';

describe('fixture search projection', () => {
  it('keeps Unicode normalization mapped to complete original scalar ranges', () => {
    const straße = normalizeWithMap('Straße');
    expect(straße.value).toBe('strasse');
    expect(straße.map[4]).toEqual({start: 4, end: 5});
    expect(straße.map[5]).toEqual({start: 4, end: 5});

    const composed = normalizeWithMap('Café 😀');
    expect(composed.value).toBe('café 😀');
    expect(composed.map[3]).toEqual({start: 3, end: 5});
    expect(composed.map[5]).toEqual({start: 6, end: 7});
  });

  it('merges only declared same-source anchors and preserves distinct hits', () => {
    const results = searchFixtureEntries(searchEntries, '卷积', 'all');
    expect(results).toHaveLength(2);
    expect(results.some((result) => result.alternateAnchors.includes('课件·第 4 页·区域 2'))).toBe(true);

    const distinct = searchFixtureEntries(searchEntries, '窗口', 'all');
    expect(distinct.length).toBeGreaterThanOrEqual(2);
    expect(new Set(distinct.map((result) => result.entry.kind)).size).toBeGreaterThan(1);
  });
});

