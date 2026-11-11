// Which of the National Park Service's videos Raygun offers. The Park Service's own works are
// not under copyright in the United States (17 U.S.C. § 105; www.nps.gov/aboutus/disclaimer.htm),
// but its library also holds partners', contractors' and other people's videos. A video is offered
// only when its credit is empty or names the National Park Service, a park or another US federal
// agency with nothing beside them but their staff's names ("NPS/Neal Herbert"), not a company, an
// institution or a work made "for" them; and its description names no other rights holder: no
// copyright, licence or permission, and no company, institution or anyone unknown as the source
// of its footage, pictures or music. No requests here.

import { PARK_CODES } from './parks';

/** The National Park Service, its programs and offices, and its rangers. */
const NPS =
  /\bNPS(?:[-‑]DSC)?\b|National Park Service|National Park System|\bNCPTT\b|National Center for Preservation Technology(?: and Training)?|Harpers Ferry Center|\bHFC\b|Denver Service Center|Natural Sounds(?: and Night Skies)?(?: Program| Division)?|\bNSNSD\b|Park History Program|Park Rangers?|U\.?\s?S\.? Park Police/giu;

/** A park unit by its designation ("Gettysburg National Military Park", "Grand Canyon NP"). */
const PARK_UNIT =
  /\b(?:National|International)\s+(?:Historical Park(?: and Preserve)?|Historic Site|Historic Trail|Historic Area|Military Park|Battlefield(?: Park| Site)?|Park(?:s| and Preserve| & Preserve)?|Monument(?: and Preserve| & Preserve)?|Memorial(?: Parkway)?|Seashore|Lakeshore|Preserve|Reserve|Recreation Area|Recreational River|Scenic River(?:ways?)?|Scenic Trail|Geologic Trail|River(?: and Recreation Area| & Recreation Area)?|Parkway|Mall)\b|\bWild (?:and|&) Scenic River\b|(?<=\p{Lu}\p{Ll}+ )(?:NP|NHP|NHS|NMP|NHT)\b/gu;

/** Other US federal agencies, whose works are in the public domain too. */
const AGENCY = new RegExp(
  [
    'U\\.?\\s?S\\.? Fish (?:and|&) Wildlife Service',
    '\\bU?S?FWS\\b',
    'U\\.?\\s?S\\.? Geological Survey',
    '\\bUSGS\\b',
    '\\bNOAA\\b',
    'National Oceanic and Atmospheric Administration',
    'National Weather Service',
    '\\bNASA\\b',
    'National Aeronautics and Space Administration',
    'National Archives(?: and Records Administration)?',
    '\\bNARA\\b',
    'Library of Congress',
    'U\\.?\\s?S\\.? Forest Service',
    '\\bUSFS\\b',
    '\\bUSDA\\b',
    'Bureau of (?:Land Management|Reclamation|Indian Affairs|Ocean Energy Management)',
    '\\bBLM\\b',
    '(?:U\\.?\\s?S\\.? )?Department of (?:the Interior|Agriculture|Defense|Energy|Transportation|Commerce|Veterans Affairs)',
    'Secretary of the Interior',
    '\\bDOI\\b',
    '(?:U\\.?\\s?S\\.?|United States) (?:Army|Navy|Air Force|Marine|Coast Guard) Band',
    '(?:U\\.?\\s?S\\.?|United States) (?:Army|Navy|Air Force|Marine Corps|Coast Guard|Space Force)',
    'Army Corps of Engineers',
    '\\bUSACE\\b',
    '\\bUSCG\\b',
    'Environmental Protection Agency',
    'Federal Highway Administration',
    '\\bFHWA\\b',
    'Federal Emergency Management Agency',
    '\\bFEMA\\b',
    'Centers for Disease Control(?: and Prevention)?',
    'National Institutes of Health',
    'Works Progress Administration',
    'Civilian Conservation Corps',
    'Tennessee Valley Authority',
    'U\\.?\\s?S\\.? Government',
  ].join('|'),
  'gu',
);

/** Words that make a part of a credit something other than the federal government. */
const NOT_FEDERAL =
  /\b(?:Foundation|Conservancy|Associations?|Friends|Trust|Society|Fund|Partners?|Partnership|Institute|University|College|School|Academy|Heritage (?:Area|Corridor)|Cooperating|Coalition|Alliance|Club|Inc|LLC|L\.L\.C|Ltd|Company|Corp|Corporation|Tribes?|Band|Nations?|State|County|City|PBS|NPR|Smithsonian)\b/iu;
/** Words naming a company or a studio unless they're an office of the Park Service's own. */
const COMPANY =
  /\b(?:Productions?|Pictures|Media|Studios?|Films?|Broadcasting|Television|TV|Radio|Publishing|Press|Network|Stream|Records|Music|Entertainment|Creative|Communications|Interpreting)\b/iu;

/** Words that say what someone did, dropped before the rest is read as a name. */
const ROLES = new Set(
  (
    'a an the of video videos photo photos photograph photographs footage film filmed filming ' +
    'produced producer production edited editing editor narrated narration narrator written ' +
    'writer script presented presenter hosted host directed director animation animated ' +
    'graphics camera cinematography audio recording recorded interview captions staff team ' +
    'crew ranger rangers volunteer volunteers intern interns interpreter photographer ' +
    'videographer courtesy made created shot dr mr mrs ms historian curator superintendent ' +
    'biologist ecologist geologist archeologist archaeologist scientist educator specialist ' +
    'technician guide docent correspondent artist author'
  ).split(' '),
);

/** Common given names: a name of two words or more is a person's when it starts with one. */
const GIVEN_NAMES = new Set(
  (
    'aaron abby abigail adam adrian adriana aidan aiden alan alana albert alex alexa alexander ' +
    'alexandra alexis alice alicia alison allen allison ally alyssa amanda amber amelia amy ' +
    'ana andrea andrew andy angel angela angelica angie anita ann anna anne annie anthony ' +
    'april ariana arthur ashley audrey austin ava barbara becky ben benjamin bernard beth ' +
    'bethany betty beverly bill billy blake bob bobby bonnie brad bradley brandon brenda brent ' +
    'brett brian briana brianna bridget brittany brooke bruce bryan caitlin caleb cameron ' +
    'carl carla carlos carmen carol caroline carolyn carrie carson casey catherine cathy ' +
    'chad charles charlie charlotte chelsea cheryl chris christian christina christine ' +
    'christopher chuck cindy claire clara clare claudia clayton cody colin colleen connie ' +
    'connor courtney craig crystal curtis cynthia daisy dale dan dana daniel danielle danny ' +
    'darren dave david dawn dean deanna debbie deborah debra dennis derek diana diane dolores ' +
    'don donald donna doris dorothy doug douglas drew dustin dylan earl ed eddie edith edward ' +
    'eileen elaine eleanor elena eli elijah elizabeth ella ellen emily emma eric erica erik ' +
    'erin ethan eugene eva evan evelyn faith frances francis frank fred gabriel gabriela gail ' +
    'gary gavin gene george gerald gina glen glenn gloria grace grant greg gregory gwen hailey ' +
    'haley hannah harold harry hayden heather heidi helen henry holly howard hunter ian isaac ' +
    'isabel isabella jack jackie jacob jacqueline jaime jake james jamie jan jane janet janice ' +
    'janis jared jasmine jason jay jean jeff jeffrey jenna jennifer jenny jeremiah jeremy ' +
    'jerry jesse jessica jill jim jimmy jo joan joann joanne joe joel john johnny jon jonathan ' +
    'jordan jorge jose joseph josh joshua joy joyce juan judith judy julia julian julie june ' +
    'justin kaitlyn kara karen kate katherine kathleen kathryn kathy katie katy kay kayla keith ' +
    'kelly kelsey ken kenneth kevin kim kimberly kirk kristen kristi kristin kristina kristine ' +
    'kristy kyle kylie lance larry laura lauren laurie lee leah leo leon leslie liam lily linda ' +
    'lindsay lindsey lisa liz logan lois lori lorraine louis louise lucas lucy luis luke lydia ' +
    'lynn mackenzie madeline madison maggie marcus margaret maria marie marilyn mario marion ' +
    'mark marsha martha martin marty mary mason matt matthew maureen max megan melanie melissa ' +
    'mia micah michael michele michelle miguel mike mildred miranda molly monica morgan nancy ' +
    'naomi natalie nathan nathaniel neal neil nicholas nick nicole noah nora norma olivia ' +
    'oscar owen pam pamela pat patricia patrick paul paula pedro peggy penny peter phil philip ' +
    'phyllis rachel ralph randy ray raymond rebecca regina renee rhonda ricardo richard rick ' +
    'ricky rita rob robert roberta robin rodney roger ron ronald rosa rose ross roy ruby russell ' +
    'ruth ryan sabrina sally sam samantha samuel sandra sara sarah scott sean seth shane shannon ' +
    'sharon shawn shelby shelly sherry shirley sierra simon sophia sophie stacey stacy stephanie ' +
    'stephen steve steven sue susan suzanne sydney tamara tammy tanya tara taylor ted teresa ' +
    'terri terry theresa thomas tiffany tim timothy tina todd tom tommy tony tracy travis trevor ' +
    'troy tyler valerie vanessa veronica vicki victor victoria vincent virginia vivian walter ' +
    'wanda warren wayne wendy wesley whitney will william willie wyatt yolanda zachary zoe ' +
    'abel agnes alberto alejandro alfred alma alvin amos andre antonio arlene armando arnold ' +
    'aubry barry beatrice bernice bert boyd brady brendan brenna brittni brittnei bryce byron ' +
    'calvin camille caryl cassandra cecilia celia chase cian clarence clifford clint clyde cole ' +
    'conrad corey cory cristina dallas damian damon dani darlene darryl dev dominic duane dwight ' +
    'edgar edna eduardo elias eliza ellie elliot elmer emilio emmett enrique ernest esther ' +
    'everett felix fernando flora floyd forrest francisco georgia gilbert gordon graham ' +
    'guadalupe guy hal harriet harvey hazel hector herbert herman homer hope hugh ida inez ' +
    'irene iris irma ivan jackson jade javier jayme jeanne jerome jesus jody joey johanna ' +
    'josephine juanita jude kai karl katelyn kellen kendra kent kerry kristyn kurt lacey ' +
    'lawrence leif leisl lena leroy lester levi lillian lloyd lonnie lorena lucia luther luz ' +
    'mabel mack mae malcolm manuel marc marcey marcia margarita marlene marshall mathew maxine ' +
    'melvin mercedes meredith mik milton mitchell nadia ned nelson nigel nina noel norman ' +
    'orlando otis pablo pearl peri perry preston priscilla quentin rafael ramon raul reed ' +
    'reggie renata rex roberto rochelle roland roman rosemary rudy sadie salvador sandy ' +
    'sangita sarinah saul sergio shay sheila sidney silvia skyler sonia spencer stacia stanley ' +
    'stella stuart sylvia terrence toby tracey trisha vera vernon vicky viola wade wallace ' +
    'wilma winston xavier yvonne zach'
  ).split(' '),
);

/** Words that are never a person's name. */
const NOT_A_NAME =
  /^(?:National|Park|Parks|Service|Services|Program|Office|Division|Department|Center|Centre|Region|Regional|Museum|Library|Archives?|University|College|School|Society|Association|Foundation|Conservancy|Trust|Friends|Fund|Media|Studios?|Productions?|Pictures|Films?|Video|Photo|Photography|Creative|Group|Company|Inc|LLC|Ltd|Corporation|Network|Television|TV|Radio|News|Press|Publishing|Partners|Institute|Project|Coalition|Alliance|Council|Commission|Committee|Board|Club|Band|Orchestra|Choir|Records|Music|Sound|Sounds|Audio|Design|Communications|Interpreting|Gallery|Church|Tribe|Nations?|County|City|State|States|Town|Village|Eastern|Western|Northern|Southern|Central|American|United|Historic|Historical|Heritage|Monument|Memorial|Trail|River|Lake|Island|Mountain|Valley|Canyon|Forest|Bay|Area|Conservation|Corps|Agency|Authority|Administration|Bureau|Government|Federal|Public|Digital|Visual|Entertainment|Labs?|Works|Workshop|Collective|Team|Staff|Crew|Volunteers|Podcast|Series|Smithsonian|History|Science|Nature|Art|Arts|Youth|Kids|Community|Stream|Mount|Mt|Fort|Saint|St|Point|Cape|Port|Mansion|House|Home|Farm|Gardens?|Hall|Castle|Hills?|Beach|Creek|Springs|Falls|Rock|Peak|Unit|Branch|Section)$/iu;

/** A word of a person's name: "Neal", "O'Brien", "VandenBerg", "Carey-Rand", "W.", "S.Steinhorst". */
const NAME_WORD =
  /^(?:\p{Lu}[\p{Ll}\p{M}'’‘ʻ]+(?:-?\p{Lu}[\p{Ll}\p{M}'’‘ʻ]+)*|\p{Lu}\.?|(?:\p{Lu}\.)+\p{Lu}[\p{Ll}\p{M}'’]+|de|da|di|du|del|der|van|von|la|le|Jr\.?|Sr\.?|II|III)$/u;
/** An initial, which only people's names have: "W.", "G", "K.Hostetler". */
const INITIAL = /^(?:\p{Lu}\.?|(?:\p{Lu}\.)+\p{Lu}[\p{Ll}\p{M}'’]+)$/u;

/** The parts a credit is made of ("NPS / Jacob W. Frank", "Annie Little, U.S. Fish and Wildlife Service"). */
const PARTS = /\s*(?:\/|\||;|,|&|\+|:|\s[-–—]\s|\(|\)|\[|\]|\bcourtesy of\b)\s*/iu;
/** Words joining the people and offices within a part ("NPS video by Jennifer Brown"). */
const JOINS =
  /(?:^|\s+)(?:and|with|for|by|from|via|at|on behalf of|funded by|in partnership with)(?:\s+|$)/iu;
/** Where a federal name was in the text, so the words around it can be read. */
const MARK = '\u{E000}';
/** Where a park's designation was: the capitalized words before it are the park's name. */
const PARK_MARK = '\u{E001}';
const PARK_NAME = new RegExp(
  `(?:\\p{Lu}[\\p{L}\\p{M}'’‘ʻ.-]*\\s+(?:(?:of|the|de|la|del)\\s+)?){1,5}${PARK_MARK}`,
  'gu',
);
const MARKS = new RegExp(`[${MARK}${PARK_MARK}]`, 'gu');

type Kind = 'federal' | 'person' | 'role' | 'other';

/** The words of a name with the roles before it dropped ("Park Ranger", "Video by"). */
const nameWords = (text: string) => {
  const words = text
    .replace(/[.,]$/, '')
    .split(/\s+/)
    .filter((word) => word !== '');
  const first = words.findIndex((word) => !ROLES.has(word.toLowerCase().replace(/[.:]$/, '')));
  return first < 0 ? [] : words.slice(first);
};

/**
 * A person's name: up to four name words, none a possessive ("Washington's"), with an initial or
 * a common given name first, so a group named in capitals ("Environmental Education Exchange")
 * isn't mistaken for someone. A surname alone counts in a credit ("NPS/Lombardi"), not in a
 * description (`strict`).
 */
const isPerson = (text: string, strict: boolean) => {
  const words = nameWords(text);
  if (words.length === 0 || words.length > 4) return false;
  const named = words.every(
    (word) => NAME_WORD.test(word) && !NOT_A_NAME.test(word) && !/['’]s$/u.test(word),
  );
  if (!named) return false;
  if (words.length === 1) return !strict;
  return (
    words.some((word) => INITIAL.test(word)) ||
    GIVEN_NAMES.has(words[0]!.toLowerCase().replace(/[.'’]/g, ''))
  );
};

/** An office's name after the Park Service's ("NPS Climate Change Response Program"). */
const isOffice = (text: string) => {
  const words = nameWords(text);
  return words.length > 0 && words.length <= 6 && words.every((word) => /^[\p{Lu}\d]/u.test(word));
};

/** What one join-free piece of a part names. */
const kindOfPiece = (piece: string, strict: boolean): Kind => {
  const text = piece.trim();
  if (nameWords(text).length === 0) return 'role';
  if (text.includes(MARK) || text.includes(PARK_MARK)) {
    // The federal name, with roles, a person's name or an office's around it.
    const rest = text.replace(MARKS, ' ').trim();
    return nameWords(rest).length === 0 || isPerson(rest, false) || isOffice(rest)
      ? 'federal'
      : 'other';
  }
  if (COMPANY.test(text)) return 'other';
  if (/^\p{Lu}{4}$/u.test(text) && PARK_CODES.has(text.toLowerCase())) return 'federal';
  return isPerson(text, strict) ? 'person' : 'other';
};

/** Made for the federal government by someone else: "Produced for NPS by …", "… for the NPS". */
const MADE_FOR = new RegExp(`\\bfor\\s+(?:the\\s+)?[${MARK}${PARK_MARK}]`, 'iu');

/** `text` with the federal names in it marked, a park's name with its designation. */
const markFederal = (text: string) =>
  text
    .normalize('NFC')
    .replace(/\s+/g, ' ')
    .replace(NPS, MARK)
    .replace(AGENCY, MARK)
    .replace(PARK_UNIT, PARK_MARK)
    .replace(PARK_NAME, PARK_MARK);

/** What a credit, or someone a description names (`strict`), is made of. */
const kindsOf = (text: string, strict: boolean): Kind[] => {
  const marked = markFederal(text)
    .trim()
    .replace(/[.\s]+$/, '')
    // "NPS-J. McLaughlin": a hyphen against a federal name parts it from the rest.
    .replace(new RegExp(`([${MARK}${PARK_MARK}])\\s*[-‑–—]\\s*`, 'gu'), '$1 / ');
  // A contractor's work: the maker keeps whatever the contract gave them.
  if (MADE_FOR.test(marked)) return ['other'];
  return marked
    .split(PARTS)
    .filter((part) => part.trim() !== '')
    .flatMap((part) =>
      NOT_FEDERAL.test(part)
        ? ['other' as const]
        : part.split(JOINS).map((piece) => kindOfPiece(piece, strict)),
    );
};

/**
 * Whether a credit names the National Park Service, a park or another federal agency, and nothing
 * else but the people who made the video for them ("NPS/Neal Herbert", "Gettysburg National
 * Military Park", "Annie Little, U.S. Fish and Wildlife Service").
 */
export const namesOnlyFederal = (text: string): boolean => {
  const kinds = kindsOf(text, false);
  return kinds.includes('federal') && !kinds.includes('other');
};

/** A copyright, licence or permission: someone else's rights. */
const RIGHTS =
  /©|\(c\)\s*(?:19|20)\d\d|\bcopyright(?:ed|s)?\b|all rights reserved|creative\s*commons|\bcc[\s-]?by\b|licen[cs]ed? (?:under|by|from|to|for)\b|under (?:a |the )?(?:\S+ ){0,4}licen[cs]e\b|licen[cs]e code|used (?:with|by|under) permission|\bpermission (?:of|from)\b|with (?:the |their |kind )*permission|royalty[- ]free|stock (?:footage|music|video)/iu;
/** A statement that something is in the public domain, which is nobody's right. */
const PUBLIC_DOMAIN_NOTE =
  /\bcopyright(?:\s+(?:info(?:rmation)?|status))?\s*:?\s*(?:none|public domain)\b|no known copyright restrictions|not (?:protected by|subject to) copyright|free of (?:known )?copyright(?: restrictions)?/giu;
/** A word of a name in a description: capitalized, a number, or a federal name marked. */
const NAME_START = `[\\p{Lu}\\d${MARK}${PARK_MARK}]`;
const NAME_REST = `[\\p{L}\\p{M}\\d'’‘ʻ.${MARK}${PARK_MARK}-]*`;
/**
 * A name in a description, as far as its capitalized words go: "Steve Broker" in "...by Steve
 * Broker of the Duck Harbor area", "the NPS Social Media" in "...by the NPS Social Media team".
 */
const NAMED = `((?:the\\s+)?${NAME_START}${NAME_REST}(?:(?:\\s*[,/&]\\s*|\\s+)(?:(?:and|the)\\s+)*${NAME_START}${NAME_REST})*)`;
/** Words before someone named as the maker of footage, pictures or music, or a partner. */
const SOURCE_WORDS = [
  'courtesy(?: of)?',
  '(?:footage|videos?|images?|photos?|photographs?|animation|music|songs?|soundtrack) by',
  '(?:music|songs?|soundtrack) from',
  '(?:produced|filmed|created|made|shot|recorded|directed|edited|narrated|presented|provided|funded|sponsored) by',
  '(?:music|songs?|soundtrack):',
  '(?:a )?(?:joint|collaboration (?:between|with)|partnership (?:between|with)|in (?:partnership|collaboration|cooperation|association) with)',
]
  .map((words) => words.replace(/\b([a-z])/g, (letter) => `[${letter}${letter.toUpperCase()}]`))
  .join('|');
const SOURCES = new RegExp(`(?:${SOURCE_WORDS})\\s+${NAMED}`, 'gu');

/** Whether a description leaves the video to its credit: it names no one else's rights. */
export const describesNoOtherRights = (description: string): boolean => {
  const text = markFederal(
    description
      .replace(/<[^>]*>/g, ' ')
      .replace(/&nbsp;/g, ' ')
      .replace(PUBLIC_DOMAIN_NOTE, ' '),
  );
  if (RIGHTS.test(text)) return false;
  return [...text.matchAll(SOURCES)].every(
    (match) => !kindsOf(match[1] ?? '', true).includes('other'),
  );
};

/** Whether a video's credit lets Raygun offer it: none, or only federal names. */
export const credited = (credit: string | null | undefined): boolean => {
  const text = (credit ?? '').trim();
  if (text === '') return true;
  if (RIGHTS.test(text.replace(PUBLIC_DOMAIN_NOTE, ' '))) return false;
  return namesOnlyFederal(text);
};
