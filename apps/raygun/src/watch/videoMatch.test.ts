import { episodeLabel, isGenericName, showsSeasons } from './episodeLabel';
import { parseTitle, sameTitle, videoTitleScore } from './videoMatch';

// The cases of `raygun_resolver::video_match`'s tests: the two parsers agree.
describe('video titles and their seasons', () => {
  it('reads the season a title names', () => {
    const cases: [string, number | null, string][] = [
      ['Overlord', null, 'overlord'],
      ['Overlord II', 2, 'overlord'],
      ['overlord ii', 2, 'overlord'],
      ['Attack on Titan Season 2', 2, 'attack on titan'],
      ['Attack on Titan: Season 2', 2, 'attack on titan'],
      ['Attack on Titan (Season 2)', 2, 'attack on titan'],
      ['Haikyu!! 2nd Season', 2, 'haikyu'],
      ['Haikyuu!! Second Season', 2, 'haikyuu'],
      ['Monogatari Series: Second Season', 2, 'monogatari series'],
      ['Sherlock Series 2', 2, 'sherlock'],
      ['Initial D Fourth Stage', 4, 'initial d'],
      ['Spy x Family Season Two', 2, 'spy x family'],
      ['Jujutsu Kaisen S2', 2, 'jujutsu kaisen'],
      ['Psycho-Pass 3: First Inspector', 3, 'psycho pass first inspector'],
      ['Date A Live V', 5, 'date a live'],
      ['Mob Psycho 100', null, 'mob psycho 100'],
      ['Mob Psycho 100 III', 3, 'mob psycho 100'],
      ['Steins;Gate 0', null, 'steins gate 0'],
      ['Mobile Suit Gundam X', null, 'mobile suit gundam x'],
      ['Detective Conan Movie 2', null, 'detective conan movie 2'],
      [
        'Is It Wrong to Try to Pick Up Girls in a Dungeon? IV Part 2',
        4,
        'is it wrong to try to pick up girls in a dungeon',
      ],
      ['Mushoku Tensei II: Isekai Ittara Honki Dasu', 2, 'mushoku tensei isekai ittara honki dasu'],
      ['進撃の巨人 第2期', 2, '進撃の巨人'],
      ['鬼灭之刃 第二季', 2, '鬼灭之刃'],
      ['Ｏｖｅｒｌｏｒｄ Ⅱ', 2, 'overlord'],
      ['나 혼자만 레벨업 시즌 2', 2, '나 혼자만 레벨업'],
    ];
    for (const [title, season, base] of cases) {
      const info = parseTitle(title);
      expect([title, info.season, info.base]).toEqual([title, season, base]);
    }
  });

  it('reads parts, final seasons, years and versions', () => {
    const marks = (title: string) => {
      const info = parseTitle(title);
      return [info.season, info.part, info.finalSeason, info.year];
    };
    expect(marks('Attack on Titan Season 3 Part 2')).toEqual([3, 2, false, null]);
    expect(marks('Mushoku Tensei: Jobless Reincarnation Season 2 Cour 2')).toEqual([
      2,
      2,
      false,
      null,
    ]);
    expect(marks('Attack on Titan: The Final Season - Final Chapters Part 2')).toEqual([
      null,
      2,
      true,
      null,
    ]);
    expect(marks('Hunter x Hunter (2011)')).toEqual([null, null, false, 2011]);
    expect(marks('SPY×FAMILY 第2クール')).toEqual([null, 2, false, null]);
    expect(parseTitle('Jujutsu Kaisen (English Dub)').base).toBe('jujutsu kaisen');
    expect(parseTitle('Fate/stay night [Unlimited Blade Works]').base).toBe(
      'fate stay night unlimited blade works',
    );
  });

  it('matches the same show and season only', () => {
    const same: [string, string][] = [
      [
        'Mushoku Tensei II: Isekai Ittara Honki Dasu',
        'Mushoku Tensei: Isekai Ittara Honki Dasu 2nd Season',
      ],
      ['Overlord II', 'OVERLORD 2nd Season'],
      ['Attack on Titan Season 2', 'Attack on Titan II'],
      ['Frieren', 'Frieren (Dub)'],
      ['Shingeki no Kyojin', 'Shingeki no Kyoujin'],
      ['Spy×Family Season 2', 'Spy x Family Season 2'],
      ['Fruits Basket 1st Season', 'Fruits Basket (2019)'],
    ];
    for (const [a, b] of same) {
      expect([a, b, sameTitle(a, b), sameTitle(b, a)]).toEqual([a, b, true, true]);
    }
    const different: [string, string][] = [
      ['Overlord', 'Overlord II'],
      ['Attack on Titan Season 3', 'Attack on Titan Season 3 Part 2'],
      ['Attack on Titan Season 4', 'Attack on Titan: The Final Season'],
      ['Fruits Basket (2001)', 'Fruits Basket (2019)'],
      ['Attack on Titan', 'Attack on Titan OVA'],
    ];
    for (const [a, b] of different) {
      expect([a, b, sameTitle(a, b), sameTitle(b, a)]).toEqual([a, b, false, false]);
    }
  });

  it('scores another season below every close match', () => {
    expect(videoTitleScore('Mushoku Tensei II', 'mushoku tensei season 2')).toBe(100);
    expect(videoTitleScore('Attack on Titan Season 2', 'attack on titan ii')).toBe(100);
    expect(videoTitleScore('Attack on Titan', 'attack on titan season 2')).toBeLessThan(60);
    expect(videoTitleScore('Attack on Titan Season 3 Part 2', 'attack on titan 2')).toBeLessThan(
      60,
    );
    // With no season asked, every season matches, the first one first.
    const first = videoTitleScore('Attack on Titan', 'attack on titan');
    const second = videoTitleScore('Attack on Titan Season 2', 'attack on titan');
    expect(first).toBe(100);
    expect(second).toBeGreaterThanOrEqual(60);
    expect(second).toBeLessThan(first);
    // Finding the same title elsewhere: no season is the first.
    expect(videoTitleScore('Attack on Titan Season 2', 'Attack on Titan', true)).toBeLessThan(60);
    expect(
      videoTitleScore('Attack on Titan Season 1', 'Attack on Titan', true),
    ).toBeGreaterThanOrEqual(60);
  });
});

describe('episode labels', () => {
  it('leaves out names that only repeat the number', () => {
    const label = (season: number | null, number: number | null, name: string) =>
      episodeLabel({ season, number, name });
    expect(label(1, 2, 'Pilot')).toBe('S1 E2 · Pilot');
    expect(label(1, 2, 'Episode 2')).toBe('S1 E2');
    expect(label(1, 2, 'S01E02')).toBe('S1 E2');
    expect(label(1, 2, 'Episode 3')).toBe('S1 E2 · Episode 3');
    expect(label(null, 5, 'Episode 5')).toBe('Episode 5');
    expect(label(null, 5, '第5話')).toBe('E5');
    expect(label(null, 5, 'The Return')).toBe('E5 · The Return');
    expect(label(null, 12.5, 'Recap')).toBe('E12.5 · Recap');
    expect(label(null, null, 'Film')).toBe('Film');
    expect(isGenericName('Ep. 05', null, 5)).toBe(true);
    expect(isGenericName('Season 2 Episode 5', 2, 5)).toBe(true);
    expect(isGenericName('Season 2 Episode 5', 1, 5)).toBe(false);
  });

  it("leaves a Kitsu entry's one season out of its labels", () => {
    const videos = [{ season: 1 }, { season: 1 }];
    expect(showsSeasons('Attack on Titan Season 2', videos)).toBe(false);
    expect(showsSeasons('Mushoku Tensei: Jobless Reincarnation Part 2', videos)).toBe(false);
    expect(showsSeasons('Attack on Titan', videos)).toBe(true);
    expect(showsSeasons('Attack on Titan', [{ season: 1 }, { season: 2 }])).toBe(true);
    expect(episodeLabel({ season: 1, number: 1, name: 'Beast Titan' }, false)).toBe(
      'E1 · Beast Titan',
    );
  });
});
