// dev.raygun.nps 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };
  var needsSetup = (reason) => {
    const host = raygun;
    if (typeof host.needsSetup === "function") host.needsSetup(reason);
    throw new Error(reason);
  };
  var query = (params) => Object.entries(params).flatMap(([name, value]) => {
    if (value === void 0 || value === null) return [];
    const values = Array.isArray(value) ? value : [value];
    return values.map((item) => `${encodeURIComponent(name)}=${encodeURIComponent(item)}`);
  }).join("&");
  var selectValue = (values, filter) => {
    const chosen = values?.[filter.key];
    const known = (value) => typeof value === "string" && filter.options.some((option) => option.value === value);
    if (known(chosen)) return chosen;
    if (known(filter.default)) return filter.default;
    return filter.options[0]?.value ?? "";
  };

  // src/parks.ts
  var PARKS = [
    ["abli", "Abraham Lincoln Birthplace National Historical Park"],
    ["acad", "Acadia National Park"],
    ["adam", "Adams National Historical Park"],
    ["afam", "African American Civil War Memorial"],
    ["afbg", "African Burial Ground National Monument"],
    ["agfo", "Agate Fossil Beds National Monument"],
    ["alka", "Ala Kahakai National Historic Trail"],
    ["alag", "Alagnak Wild River"],
    ["anch", "Alaska Public Lands"],
    ["alca", "Alcatraz Island"],
    ["aleu", "Aleutian Islands World War II National Historic Area"],
    ["alfl", "Alibates Flint Quarries National Monument"],
    ["alpo", "Allegheny Portage Railroad National Historic Site"],
    ["amch", "Amache National Historic Site"],
    ["amme", "American Memorial Park"],
    ["amis", "Amistad National Recreation Area"],
    ["anac", "Anacostia Park"],
    ["ande", "Andersonville National Historic Site"],
    ["anjo", "Andrew Johnson National Historic Site"],
    ["ania", "Aniakchak National Monument & Preserve"],
    ["anti", "Antietam National Battlefield"],
    ["apis", "Apostle Islands National Lakeshore"],
    ["appa", "Appalachian National Scenic Trail"],
    ["apco", "Appomattox Court House National Historical Park"],
    ["arch", "Arches National Park"],
    ["arpo", "Arkansas Post National Memorial"],
    ["arho", "Arlington House, The Robert E. Lee Memorial"],
    ["asis", "Assateague Island National Seashore"],
    ["azru", "Aztec Ruins National Monument"],
    ["badl", "Badlands National Park"],
    ["bawa", "Baltimore-Washington Parkway"],
    ["band", "Bandelier National Monument"],
    ["bepa", "Belmont-Paul Women's Equality National Monument"],
    ["beol", "Bent's Old Fort National Historic Site"],
    ["bela", "Bering Land Bridge National Preserve"],
    ["bibe", "Big Bend National Park"],
    ["bicy", "Big Cypress National Preserve"],
    ["biho", "Big Hole National Battlefield"],
    ["biso", "Big South Fork National River & Recreation Area"],
    ["bith", "Big Thicket National Preserve"],
    ["bica", "Bighorn Canyon National Recreation Area"],
    ["bicr", "Birmingham Civil Rights National Monument"],
    ["bisc", "Biscayne National Park"],
    ["blca", "Black Canyon Of The Gunnison National Park"],
    ["blrv", "Blackstone River Valley National Historical Park"],
    ["blsc", "Blackwell School National Historic Site"],
    ["blri", "Blue Ridge Parkway"],
    ["blue", "Bluestone National Scenic River"],
    ["bowa", "Booker T. Washington National Monument"],
    ["boaf", "Boston African American National Historic Site"],
    ["boha", "Boston Harbor Islands National Recreation Area"],
    ["bost", "Boston National Historical Park"],
    ["brcr", "Brices Cross Roads National Battlefield Site"],
    ["brvb", "Brown v. Board of Education National Historical Park"],
    ["brca", "Bryce Canyon National Park"],
    ["buis", "Buck Island Reef National Monument"],
    ["buff", "Buffalo National River"],
    ["buov", "Butterfield Overland National Historic Trail"],
    ["cabr", "Cabrillo National Monument"],
    ["cali", "California National Historic Trail"],
    ["cane", "Camp Nelson National Monument"],
    ["cana", "Canaveral National Seashore"],
    ["cari", "Cane River Creole National Historical Park"],
    ["cach", "Canyon de Chelly National Monument"],
    ["cany", "Canyonlands National Park"],
    ["caco", "Cape Cod National Seashore"],
    ["caha", "Cape Hatteras National Seashore"],
    ["came", "Cape Henry Memorial Part of Colonial National Historical Park"],
    ["cakr", "Cape Krusenstern National Monument"],
    ["calo", "Cape Lookout National Seashore"],
    ["cahi", "Capitol Hill Parks"],
    ["care", "Capitol Reef National Park"],
    ["cajo", "Captain John Smith Chesapeake National Historic Trail"],
    ["cavo", "Capulin Volcano National Monument"],
    ["carl", "Carl Sandburg Home National Historic Site"],
    ["cibs", "Carlisle Federal Indian Boarding School National Monument"],
    ["cave", "Carlsbad Caverns National Park"],
    ["cawo", "Carter G. Woodson Home National Historic Site"],
    ["cagr", "Casa Grande Ruins National Monument"],
    ["casa", "Castillo de San Marcos National Monument"],
    ["cacl", "Castle Clinton National Monument"],
    ["camo", "Castle Mountains National Monument"],
    ["cato", "Catoctin Mountain Park"],
    ["cebr", "Cedar Breaks National Monument"],
    ["cebe", "Cedar Creek & Belle Grove National Historical Park"],
    ["chcu", "Chaco Culture National Historical Park"],
    ["cham", "Chamizal National Memorial"],
    ["chis", "Channel Islands National Park"],
    ["chpi", "Charles Pinckney National Historic Site"],
    ["chyo", "Charles Young Buffalo Soldiers National Monument"],
    ["chat", "Chattahoochee River National Recreation Area"],
    ["choh", "Chesapeake & Ohio Canal National Historical Park"],
    ["cbpo", "Chesapeake Bay"],
    ["chch", "Chickamauga & Chattanooga National Military Park"],
    ["chic", "Chickasaw National Recreation Area"],
    ["chir", "Chiricahua National Park"],
    ["chri", "Christiansted National Historic Site"],
    ["ciro", "City Of Rocks National Reserve"],
    ["cwdw", "Civil War Defenses of Washington"],
    ["clba", "Clara Barton National Historic Site"],
    ["colo", "Colonial National Historical Park"],
    ["colm", "Colorado National Monument"],
    ["colt", "Coltsville National Historical Park"],
    ["cong", "Congaree National Park"],
    ["coga", "Constitution Gardens"],
    ["coro", "Coronado National Memorial"],
    ["cowp", "Cowpens National Battlefield"],
    ["crla", "Crater Lake National Park"],
    ["crmo", "Craters Of The Moon National Monument & Preserve"],
    ["cuga", "Cumberland Gap National Historical Park"],
    ["cuis", "Cumberland Island National Seashore"],
    ["cure", "Curecanti National Recreation Area"],
    ["cuva", "Cuyahoga Valley National Park"],
    ["cech", "César E. Chávez National Monument"],
    ["daav", "Dayton Aviation Heritage National Historical Park"],
    ["deso", "De Soto National Memorial"],
    ["deva", "Death Valley National Park"],
    ["dewa", "Delaware Water Gap National Recreation Area"],
    ["dena", "Denali National Park & Preserve"],
    ["dsds", "Desert Shield and Desert Storm Memorial"],
    ["depo", "Devils Postpile National Monument"],
    ["deto", "Devils Tower National Monument"],
    ["dino", "Dinosaur National Monument"],
    ["drto", "Dry Tortugas National Park"],
    ["ddem", "Dwight D. Eisenhower Memorial"],
    ["ebla", "Ebey's Landing National Historical Reserve"],
    ["edal", "Edgar Allan Poe National Historic Site"],
    ["efmo", "Effigy Mounds National Monument"],
    ["eise", "Eisenhower National Historic Site"],
    ["elte", "El Camino Real de los Tejas National Historic Trail"],
    ["elca", "El Camino Real de Tierra Adentro National Historic Trail"],
    ["elma", "El Malpais National Monument"],
    ["elmo", "El Morro National Monument"],
    ["elro", "Eleanor Roosevelt National Historic Site"],
    ["elis", "Ellis Island Part of Statue of Liberty National Monument"],
    ["till", "Emmett Till and Mamie Till-Mobley National Monument"],
    ["euon", "Eugene O'Neill National Historic Site"],
    ["ever", "Everglades National Park"],
    ["feha", "Federal Hall National Memorial"],
    ["fiis", "Fire Island National Seashore"],
    ["fila", "First Ladies National Historic Site"],
    ["frst", "First State National Historical Park"],
    ["flni", "Flight 93 National Memorial"],
    ["flfo", "Florissant Fossil Beds National Monument"],
    ["foth", "Ford's Theatre National Historic Site"],
    ["fobo", "Fort Bowie National Historic Site"],
    ["foda", "Fort Davis National Historic Site"],
    ["fodo", "Fort Donelson National Battlefield"],
    ["fodu", "Fort Dupont Park"],
    ["fofo", "Fort Foote Park"],
    ["fofr", "Fort Frederica National Monument"],
    ["fola", "Fort Laramie National Historic Site"],
    ["fols", "Fort Larned National Historic Site"],
    ["foma", "Fort Matanzas National Monument"],
    ["fomc", "Fort McHenry National Monument and Historic Shrine"],
    ["fomr", "Fort Monroe National Monument"],
    ["fone", "Fort Necessity National Battlefield"],
    ["fopo", "Fort Point National Historic Site"],
    ["fopu", "Fort Pulaski National Monument"],
    ["fora", "Fort Raleigh National Historic Site"],
    ["fosc", "Fort Scott National Historic Site"],
    ["fosm", "Fort Smith National Historic Site"],
    ["fost", "Fort Stanwix National Monument"],
    ["fosu", "Fort Sumter and Fort Moultrie National Historical Park"],
    ["foun", "Fort Union National Monument"],
    ["fous", "Fort Union Trading Post National Historic Site"],
    ["fova", "Fort Vancouver National Historic Site"],
    ["fowa", "Fort Washington Park"],
    ["fobu", "Fossil Butte National Monument"],
    ["frpe", "Frances Perkins National Monument"],
    ["frde", "Franklin Delano Roosevelt Memorial"],
    ["frdo", "Frederick Douglass National Historic Site"],
    ["frla", "Frederick Law Olmsted National Historic Site"],
    ["frsp", "Fredericksburg & Spotsylvania National Military Park"],
    ["frri", "Freedom Riders National Monument"],
    ["frhi", "Friendship Hill National Historic Site"],
    ["gaar", "Gates Of The Arctic National Park & Preserve"],
    ["jeff", "Gateway Arch National Park"],
    ["gate", "Gateway National Recreation Area"],
    ["gari", "Gauley River National Recreation Area"],
    ["gegr", "General Grant National Memorial"],
    ["gero", "George Rogers Clark National Historical Park"],
    ["gewa", "George Washington Birthplace National Monument"],
    ["gwca", "George Washington Carver National Monument"],
    ["gwmp", "George Washington Memorial Parkway"],
    ["gett", "Gettysburg National Military Park"],
    ["gicl", "Gila Cliff Dwellings National Monument"],
    ["glba", "Glacier Bay National Park & Preserve"],
    ["glac", "Glacier National Park"],
    ["glca", "Glen Canyon National Recreation Area"],
    ["glec", "Glen Echo Park"],
    ["glde", "Gloria Dei Church National Historic Site"],
    ["goga", "Golden Gate National Recreation Area"],
    ["gosp", "Golden Spike National Historical Park"],
    ["gois", "Governors Island National Monument"],
    ["grca", "Grand Canyon National Park"],
    ["para", "Grand Canyon-Parashant National Monument"],
    ["grpo", "Grand Portage National Monument"],
    ["grte", "Grand Teton National Park"],
    ["grko", "Grant-Kohrs Ranch National Historic Site"],
    ["grba", "Great Basin National Park"],
    ["greg", "Great Egg Harbor River"],
    ["grfa", "Great Falls Park"],
    ["grsa", "Great Sand Dunes National Park & Preserve"],
    ["grsm", "Great Smoky Mountains National Park"],
    ["grsp", "Green Springs"],
    ["gree", "Greenbelt Park"],
    ["gumo", "Guadalupe Mountains National Park"],
    ["guco", "Guilford Courthouse National Military Park"],
    ["guis", "Gulf Islands National Seashore"],
    ["hafo", "Hagerman Fossil Beds National Monument"],
    ["hale", "Haleakalā National Park"],
    ["hagr", "Hamilton Grange National Memorial"],
    ["hamp", "Hampton National Historic Site"],
    ["haha", "Harmony Hall"],
    ["hafe", "Harpers Ferry National Historical Park"],
    ["hart", "Harriet Tubman National Historical Park"],
    ["hatu", "Harriet Tubman Underground Railroad National Historical Park"],
    ["hstr", "Harry S Truman National Historic Site"],
    ["havo", "Hawaiʻi Volcanoes National Park"],
    ["heho", "Herbert Hoover National Historic Site"],
    ["jame", "Historic Jamestowne Part of Colonial National Historical Park"],
    ["hofr", "Home Of Franklin D Roosevelt National Historic Site"],
    ["home", "Homestead National Historical Park"],
    ["hono", "Honouliuli National Historic Site"],
    ["hocu", "Hopewell Culture National Historical Park"],
    ["hofu", "Hopewell Furnace National Historic Site"],
    ["hobe", "Horseshoe Bend National Military Park"],
    ["hosp", "Hot Springs National Park"],
    ["hove", "Hovenweep National Monument"],
    ["hutr", "Hubbell Trading Post National Historic Site"],
    ["iafl", "Ice Age Floods National Geologic Trail"],
    ["iatr", "Ice Age National Scenic Trail"],
    ["inde", "Independence National Historical Park"],
    ["indu", "Indiana Dunes National Park"],
    ["isro", "Isle Royale National Park"],
    ["inup", "Iñupiat Heritage Center"],
    ["jaga", "James A Garfield National Historic Site"],
    ["jela", "Jean Lafitte National Historical Park and Preserve"],
    ["jeca", "Jewel Cave National Monument"],
    ["jica", "Jimmy Carter National Historical Park"],
    ["joda", "John Day Fossil Beds National Monument"],
    ["jofi", "John Fitzgerald Kennedy National Historic Site"],
    ["jomu", "John Muir National Historic Site"],
    ["jofl", "Johnstown Flood National Memorial"],
    ["jotr", "Joshua Tree National Park"],
    ["juba", "Juan Bautista de Anza National Historic Trail"],
    ["kala", "Kalaupapa National Historical Park"],
    ["kaho", "Kaloko-Honokōhau National Historical Park"],
    ["kaww", "Katahdin Woods and Waters National Monument"],
    ["katm", "Katmai National Park & Preserve"],
    ["kefj", "Kenai Fjords National Park"],
    ["keaq", "Kenilworth Park & Aquatic Gardens"],
    ["kemo", "Kennesaw Mountain National Battlefield Park"],
    ["kewe", "Keweenaw National Historical Park"],
    ["kimo", "Kings Mountain National Military Park"],
    ["klse", "Klondike Gold Rush - Seattle Unit National Historical Park"],
    ["klgo", "Klondike Gold Rush National Historical Park"],
    ["knri", "Knife River Indian Villages National Historic Site"],
    ["kova", "Kobuk Valley National Park"],
    ["kowa", "Korean War Veterans Memorial"],
    ["lacl", "Lake Clark National Park & Preserve"],
    ["lake", "Lake Mead National Recreation Area"],
    ["lamr", "Lake Meredith National Recreation Area"],
    ["laro", "Lake Roosevelt National Recreation Area"],
    ["lavo", "Lassen Volcanic National Park"],
    ["labe", "Lava Beds National Monument"],
    ["lyba", "LBJ Memorial Grove on the Potomac"],
    ["lecl", "Lewis & Clark National Historic Trail"],
    ["lewi", "Lewis and Clark National Historical Park"],
    ["libo", "Lincoln Boyhood National Memorial"],
    ["liho", "Lincoln Home National Historic Site"],
    ["linc", "Lincoln Memorial"],
    ["libi", "Little Bighorn Battlefield National Monument"],
    ["liri", "Little River Canyon National Preserve"],
    ["chsc", "Little Rock Central High School National Historic Site"],
    ["long", "Longfellow House Washington's Headquarters National Historic Site"],
    ["lowe", "Lowell National Historical Park"],
    ["lode", "Lower Delaware National Wild and Scenic River"],
    ["loea", "Lower East Side Tenement Museum National Historic Site"],
    ["lyjo", "Lyndon B Johnson National Historical Park"],
    ["mawa", "Maggie L Walker National Historic Site"],
    ["maac", "Maine Acadian Culture"],
    ["maca", "Mammoth Cave National Park"],
    ["mana", "Manassas National Battlefield Park"],
    ["mapr", "Manhattan Project National Historical Park"],
    ["manz", "Manzanar National Historic Site"],
    ["mabi", "Marsh - Billings - Rockefeller National Historical Park"],
    ["mlkm", "Martin Luther King, Jr. Memorial"],
    ["malu", "Martin Luther King, Jr. National Historical Park"],
    ["mava", "Martin Van Buren National Historic Site"],
    ["mamc", "Mary McLeod Bethune Council House National Historic Site"],
    ["memy", "Medgar and Myrlie Evers Home National Monument"],
    ["meve", "Mesa Verde National Park"],
    ["misp", "Mill Springs Battlefield National Monument"],
    ["miin", "Minidoka National Historic Site"],
    ["mima", "Minute Man National Historical Park"],
    ["mimi", "Minuteman Missile National Historic Site"],
    ["miss", "Mississippi National River & Recreation Area"],
    ["mnrr", "Missouri National Recreational River"],
    ["moja", "Mojave National Preserve"],
    ["mono", "Monocacy National Battlefield"],
    ["moca", "Montezuma Castle National Monument"],
    ["mocr", "Moores Creek National Battlefield"],
    ["mopi", "Mormon Pioneer National Historic Trail"],
    ["morr", "Morristown National Historical Park"],
    ["mora", "Mount Rainier National Park"],
    ["moru", "Mount Rushmore National Memorial"],
    ["muwo", "Muir Woods National Monument"],
    ["natc", "Natchez National Historical Park"],
    ["natt", "Natchez Trace National Scenic Trail"],
    ["natr", "Natchez Trace Parkway"],
    ["nace", "National Capital Parks-East"],
    ["nama", "National Mall and Memorial Parks"],
    ["npsa", "National Park of American Samoa"],
    ["npnh", "National Parks of New York Harbor"],
    ["nabr", "Natural Bridges National Monument"],
    ["nava", "Navajo National Monument"],
    ["nebe", "New Bedford Whaling National Historical Park"],
    ["neen", "New England National Scenic Trail"],
    ["pine", "New Jersey Pinelands National Reserve"],
    ["jazz", "New Orleans Jazz National Historical Park"],
    ["neph", "New Philadelphia National Historic Site"],
    ["neri", "New River Gorge National Park & Preserve"],
    ["nepe", "Nez Perce National Historical Park"],
    ["nico", "Nicodemus National Historic Site"],
    ["nisi", "Ninety Six National Historic Site"],
    ["niob", "Niobrara National Scenic River"],
    ["noat", "Noatak National Preserve"],
    ["noca", "North Cascades National Park"],
    ["noco", "North Country National Scenic Trail"],
    ["obed", "Obed Wild & Scenic River"],
    ["ocmu", "Ocmulgee Mounds National Historical Park"],
    ["okci", "Oklahoma City National Memorial"],
    ["olsp", "Old Spanish National Historic Trail"],
    ["olym", "Olympic National Park"],
    ["orca", "Oregon Caves National Monument & Preserve"],
    ["oreg", "Oregon National Historic Trail"],
    ["orpi", "Organ Pipe Cactus National Monument"],
    ["ovvi", "Overmountain Victory National Historic Trail"],
    ["oxhi", "Oxon Cove Park & Oxon Hill Farm"],
    ["ozar", "Ozark National Scenic Riverways"],
    ["pais", "Padre Island National Seashore"],
    ["paal", "Palo Alto Battlefield National Historical Park"],
    ["pagr", "Paterson Great Falls National Historical Park"],
    ["peri", "Pea Ridge National Military Park"],
    ["valr", "Pearl Harbor National Memorial"],
    ["peco", "Pecos National Historical Park"],
    ["paav", "Pennsylvania Avenue"],
    ["pevi", "Perry's Victory & International Peace Memorial"],
    ["pete", "Petersburg National Battlefield"],
    ["pefo", "Petrified Forest National Park"],
    ["petr", "Petroglyph National Monument"],
    ["piro", "Pictured Rocks National Lakeshore"],
    ["pinn", "Pinnacles National Park"],
    ["pisp", "Pipe Spring National Monument"],
    ["pipe", "Pipestone National Monument"],
    ["pisc", "Piscataway Park"],
    ["pore", "Point Reyes National Seashore"],
    ["poex", "Pony Express National Historic Trail"],
    ["poch", "Port Chicago Naval Magazine National Memorial"],
    ["pohe", "Potomac Heritage National Scenic Trail"],
    ["popo", "Poverty Point National Monument"],
    ["wicl", "President William Jefferson Clinton Birthplace Home National Historic Site"],
    ["prsf", "Presidio of San Francisco"],
    ["prwi", "Prince William Forest Park"],
    ["pull", "Pullman National Historical Park"],
    ["puho", "Puʻuhonua o Hōnaunau National Historical Park"],
    ["puhe", "Puʻukoholā Heiau National Historic Site"],
    ["rabr", "Rainbow Bridge National Monument"],
    ["reer", "Reconstruction Era National Historical Park"],
    ["redw", "Redwood National and State Parks"],
    ["rich", "Richmond National Battlefield Park"],
    ["rigr", "Rio Grande Wild & Scenic River"],
    ["rira", "River Raisin National Battlefield Park"],
    ["rocr", "Rock Creek Park"],
    ["romo", "Rocky Mountain National Park"],
    ["rowi", "Roger Williams National Memorial"],
    ["roca", "Roosevelt Campobello International Park"],
    ["rori", "Rosie the Riveter WWII Home Front National Historical Park"],
    ["ruca", "Russell Cave National Monument"],
    ["sahi", "Sagamore Hill National Historic Site"],
    ["sagu", "Saguaro National Park"],
    ["sacr", "Saint Croix Island International Historic Site"],
    ["sacn", "Saint Croix National Scenic Riverway"],
    ["sapa", "Saint Paul's Church National Historic Site"],
    ["saga", "Saint-Gaudens National Historical Park"],
    ["sama", "Salem Maritime National Historical Park"],
    ["sapu", "Salinas Pueblo Missions National Monument"],
    ["sari", "Salt River Bay National Historical Park and Ecological Preserve"],
    ["saan", "San Antonio Missions National Historical Park"],
    ["safr", "San Francisco Maritime National Historical Park"],
    ["sajh", "San Juan Island National Historical Park"],
    ["saju", "San Juan National Historic Site"],
    ["sand", "Sand Creek Massacre National Historic Site"],
    ["safe", "Santa Fe National Historic Trail"],
    ["samo", "Santa Monica Mountains National Recreation Area"],
    ["sara", "Saratoga National Historical Park"],
    ["sair", "Saugus Iron Works National Historic Site"],
    ["scbl", "Scotts Bluff National Monument"],
    ["semo", "Selma To Montgomery National Historic Trail"],
    ["seki", "Sequoia & Kings Canyon National Parks"],
    ["shen", "Shenandoah National Park"],
    ["shil", "Shiloh National Military Park"],
    ["sitk", "Sitka National Historical Park"],
    ["slbe", "Sleeping Bear Dunes National Lakeshore"],
    ["spra", "Springfield 1908 Race Riot National Monument"],
    ["spar", "Springfield Armory National Historic Site"],
    ["stsp", "Star-Spangled Banner National Historic Trail"],
    ["stli", "Statue Of Liberty National Monument"],
    ["stge", "Ste. Geneviève National Historical Park"],
    ["stea", "Steamtown National Historic Site"],
    ["stri", "Stones River National Battlefield"],
    ["ston", "Stonewall National Monument"],
    ["sucr", "Sunset Crater Volcano National Monument"],
    ["tapr", "Tallgrass Prairie National Preserve"],
    ["thko", "Thaddeus Kosciuszko National Memorial"],
    ["whho", "The White House and President's Park"],
    ["thrb", "Theodore Roosevelt Birthplace National Historic Site"],
    ["thri", "Theodore Roosevelt Inaugural National Historic Site"],
    ["this", "Theodore Roosevelt Island"],
    ["thro", "Theodore Roosevelt National Park"],
    ["thco", "Thomas Cole National Historic Site"],
    ["edis", "Thomas Edison National Historical Park"],
    ["thje", "Thomas Jefferson Memorial"],
    ["thst", "Thomas Stone National Historic Site"],
    ["tica", "Timpanogos Cave National Monument"],
    ["timu", "Timucuan Ecological & Historic Preserve"],
    ["tont", "Tonto National Monument"],
    ["tosy", "Touro Synagogue National Historic Site"],
    ["trte", "Trail Of Tears National Historic Trail"],
    ["tule", "Tule Lake National Monument"],
    ["tusk", "Tule Springs Fossil Beds National Monument"],
    ["tuma", "Tumacácori National Historical Park"],
    ["tupe", "Tupelo National Battlefield"],
    ["tuai", "Tuskegee Airmen National Historic Site"],
    ["tuin", "Tuskegee Institute National Historic Site"],
    ["tuzi", "Tuzigoot National Monument"],
    ["ulsg", "Ulysses S Grant National Historic Site"],
    ["upde", "Upper Delaware Scenic & Recreational River"],
    ["vall", "Valles Caldera National Preserve"],
    ["vafo", "Valley Forge National Historical Park"],
    ["vama", "Vanderbilt Mansion National Historic Site"],
    ["vick", "Vicksburg National Military Park"],
    ["vive", "Vietnam Veterans Memorial"],
    ["vicr", "Virgin Islands Coral Reef National Monument"],
    ["viis", "Virgin Islands National Park"],
    ["voya", "Voyageurs National Park"],
    ["waco", "Waco Mammoth National Monument"],
    ["waca", "Walnut Canyon National Monument"],
    ["wapa", "War In The Pacific National Historical Park"],
    ["wamo", "Washington Monument"],
    ["waro", "Washington-Rochambeau Revolutionary Route National Historic Trail"],
    ["waba", "Washita Battlefield National Historic Site"],
    ["wefa", "Weir Farm National Historical Park"],
    ["whis", "Whiskeytown National Recreation Area"],
    ["whsa", "White Sands National Park"],
    ["whmi", "Whitman Mission National Historic Site"],
    ["wiho", "William Howard Taft National Historic Site"],
    ["wicr", "Wilson's Creek National Battlefield"],
    ["wica", "Wind Cave National Park"],
    ["wing", "Wing Luke Museum Affiliated Area"],
    ["wotr", "Wolf Trap National Park for the Performing Arts"],
    ["wori", "Women's Rights National Historical Park"],
    ["wwim", "World War I Memorial"],
    ["wwii", "World War II Memorial"],
    ["wrst", "Wrangell - St Elias National Park & Preserve"],
    ["wrbr", "Wright Brothers National Memorial"],
    ["wupa", "Wupatki National Monument"],
    ["yell", "Yellowstone National Park"],
    ["york", "Yorktown Battlefield Part of Colonial National Historical Park"],
    ["yose", "Yosemite National Park"],
    ["yuho", "Yucca House National Monument"],
    ["yuch", "Yukon - Charley Rivers National Preserve"],
    ["zion", "Zion National Park"]
  ];
  var PARK_CODES = new Set(PARKS.map(([code]) => code));
  var STATES = [
    ["AL", "Alabama"],
    ["AK", "Alaska"],
    ["AS", "American Samoa"],
    ["AZ", "Arizona"],
    ["AR", "Arkansas"],
    ["CA", "California"],
    ["CO", "Colorado"],
    ["CT", "Connecticut"],
    ["DE", "Delaware"],
    ["DC", "District of Columbia"],
    ["FL", "Florida"],
    ["GA", "Georgia"],
    ["GU", "Guam"],
    ["HI", "Hawaii"],
    ["ID", "Idaho"],
    ["IL", "Illinois"],
    ["IN", "Indiana"],
    ["IA", "Iowa"],
    ["KS", "Kansas"],
    ["KY", "Kentucky"],
    ["LA", "Louisiana"],
    ["ME", "Maine"],
    ["MD", "Maryland"],
    ["MA", "Massachusetts"],
    ["MI", "Michigan"],
    ["MN", "Minnesota"],
    ["MS", "Mississippi"],
    ["MO", "Missouri"],
    ["MT", "Montana"],
    ["NE", "Nebraska"],
    ["NV", "Nevada"],
    ["NH", "New Hampshire"],
    ["NJ", "New Jersey"],
    ["NM", "New Mexico"],
    ["NY", "New York"],
    ["NC", "North Carolina"],
    ["ND", "North Dakota"],
    ["MP", "Northern Mariana Islands"],
    ["OH", "Ohio"],
    ["OK", "Oklahoma"],
    ["OR", "Oregon"],
    ["PA", "Pennsylvania"],
    ["PR", "Puerto Rico"],
    ["RI", "Rhode Island"],
    ["SC", "South Carolina"],
    ["SD", "South Dakota"],
    ["TN", "Tennessee"],
    ["TX", "Texas"],
    ["VI", "U.S. Virgin Islands"],
    ["UT", "Utah"],
    ["VT", "Vermont"],
    ["VA", "Virginia"],
    ["WA", "Washington"],
    ["WV", "West Virginia"],
    ["WI", "Wisconsin"],
    ["WY", "Wyoming"]
  ];

  // src/rights.ts
  var NPS = /\bNPS(?:[-‑]DSC)?\b|National Park Service|National Park System|\bNCPTT\b|National Center for Preservation Technology(?: and Training)?|Harpers Ferry Center|\bHFC\b|Denver Service Center|Natural Sounds(?: and Night Skies)?(?: Program| Division)?|\bNSNSD\b|Park History Program|Park Rangers?|U\.?\s?S\.? Park Police/giu;
  var PARK_UNIT = /\b(?:National|International)\s+(?:Historical Park(?: and Preserve)?|Historic Site|Historic Trail|Historic Area|Military Park|Battlefield(?: Park| Site)?|Park(?:s| and Preserve| & Preserve)?|Monument(?: and Preserve| & Preserve)?|Memorial(?: Parkway)?|Seashore|Lakeshore|Preserve|Reserve|Recreation Area|Recreational River|Scenic River(?:ways?)?|Scenic Trail|Geologic Trail|River(?: and Recreation Area| & Recreation Area)?|Parkway|Mall)\b|\bWild (?:and|&) Scenic River\b|(?<=\p{Lu}\p{Ll}+ )(?:NP|NHP|NHS|NMP|NHT)\b/gu;
  var AGENCY = new RegExp(
    [
      "U\\.?\\s?S\\.? Fish (?:and|&) Wildlife Service",
      "\\bU?S?FWS\\b",
      "U\\.?\\s?S\\.? Geological Survey",
      "\\bUSGS\\b",
      "\\bNOAA\\b",
      "National Oceanic and Atmospheric Administration",
      "National Weather Service",
      "\\bNASA\\b",
      "National Aeronautics and Space Administration",
      "National Archives(?: and Records Administration)?",
      "\\bNARA\\b",
      "Library of Congress",
      "U\\.?\\s?S\\.? Forest Service",
      "\\bUSFS\\b",
      "\\bUSDA\\b",
      "Bureau of (?:Land Management|Reclamation|Indian Affairs|Ocean Energy Management)",
      "\\bBLM\\b",
      "(?:U\\.?\\s?S\\.? )?Department of (?:the Interior|Agriculture|Defense|Energy|Transportation|Commerce|Veterans Affairs)",
      "Secretary of the Interior",
      "\\bDOI\\b",
      "(?:U\\.?\\s?S\\.?|United States) (?:Army|Navy|Air Force|Marine|Coast Guard) Band",
      "(?:U\\.?\\s?S\\.?|United States) (?:Army|Navy|Air Force|Marine Corps|Coast Guard|Space Force)",
      "Army Corps of Engineers",
      "\\bUSACE\\b",
      "\\bUSCG\\b",
      "Environmental Protection Agency",
      "Federal Highway Administration",
      "\\bFHWA\\b",
      "Federal Emergency Management Agency",
      "\\bFEMA\\b",
      "Centers for Disease Control(?: and Prevention)?",
      "National Institutes of Health",
      "Works Progress Administration",
      "Civilian Conservation Corps",
      "Tennessee Valley Authority",
      "U\\.?\\s?S\\.? Government"
    ].join("|"),
    "gu"
  );
  var NOT_FEDERAL = /\b(?:Foundation|Conservancy|Associations?|Friends|Trust|Society|Fund|Partners?|Partnership|Institute|University|College|School|Academy|Heritage (?:Area|Corridor)|Cooperating|Coalition|Alliance|Club|Inc|LLC|L\.L\.C|Ltd|Company|Corp|Corporation|Tribes?|Band|Nations?|State|County|City|PBS|NPR|Smithsonian)\b/iu;
  var COMPANY = /\b(?:Productions?|Pictures|Media|Studios?|Films?|Broadcasting|Television|TV|Radio|Publishing|Press|Network|Stream|Records|Music|Entertainment|Creative|Communications|Interpreting)\b/iu;
  var ROLES = new Set(
    "a an the of video videos photo photos photograph photographs footage film filmed filming produced producer production edited editing editor narrated narration narrator written writer script presented presenter hosted host directed director animation animated graphics camera cinematography audio recording recorded interview captions staff team crew ranger rangers volunteer volunteers intern interns interpreter photographer videographer courtesy made created shot dr mr mrs ms historian curator superintendent biologist ecologist geologist archeologist archaeologist scientist educator specialist technician guide docent correspondent artist author".split(" ")
  );
  var GIVEN_NAMES = new Set(
    "aaron abby abigail adam adrian adriana aidan aiden alan alana albert alex alexa alexander alexandra alexis alice alicia alison allen allison ally alyssa amanda amber amelia amy ana andrea andrew andy angel angela angelica angie anita ann anna anne annie anthony april ariana arthur ashley audrey austin ava barbara becky ben benjamin bernard beth bethany betty beverly bill billy blake bob bobby bonnie brad bradley brandon brenda brent brett brian briana brianna bridget brittany brooke bruce bryan caitlin caleb cameron carl carla carlos carmen carol caroline carolyn carrie carson casey catherine cathy chad charles charlie charlotte chelsea cheryl chris christian christina christine christopher chuck cindy claire clara clare claudia clayton cody colin colleen connie connor courtney craig crystal curtis cynthia daisy dale dan dana daniel danielle danny darren dave david dawn dean deanna debbie deborah debra dennis derek diana diane dolores don donald donna doris dorothy doug douglas drew dustin dylan earl ed eddie edith edward eileen elaine eleanor elena eli elijah elizabeth ella ellen emily emma eric erica erik erin ethan eugene eva evan evelyn faith frances francis frank fred gabriel gabriela gail gary gavin gene george gerald gina glen glenn gloria grace grant greg gregory gwen hailey haley hannah harold harry hayden heather heidi helen henry holly howard hunter ian isaac isabel isabella jack jackie jacob jacqueline jaime jake james jamie jan jane janet janice janis jared jasmine jason jay jean jeff jeffrey jenna jennifer jenny jeremiah jeremy jerry jesse jessica jill jim jimmy jo joan joann joanne joe joel john johnny jon jonathan jordan jorge jose joseph josh joshua joy joyce juan judith judy julia julian julie june justin kaitlyn kara karen kate katherine kathleen kathryn kathy katie katy kay kayla keith kelly kelsey ken kenneth kevin kim kimberly kirk kristen kristi kristin kristina kristine kristy kyle kylie lance larry laura lauren laurie lee leah leo leon leslie liam lily linda lindsay lindsey lisa liz logan lois lori lorraine louis louise lucas lucy luis luke lydia lynn mackenzie madeline madison maggie marcus margaret maria marie marilyn mario marion mark marsha martha martin marty mary mason matt matthew maureen max megan melanie melissa mia micah michael michele michelle miguel mike mildred miranda molly monica morgan nancy naomi natalie nathan nathaniel neal neil nicholas nick nicole noah nora norma olivia oscar owen pam pamela pat patricia patrick paul paula pedro peggy penny peter phil philip phyllis rachel ralph randy ray raymond rebecca regina renee rhonda ricardo richard rick ricky rita rob robert roberta robin rodney roger ron ronald rosa rose ross roy ruby russell ruth ryan sabrina sally sam samantha samuel sandra sara sarah scott sean seth shane shannon sharon shawn shelby shelly sherry shirley sierra simon sophia sophie stacey stacy stephanie stephen steve steven sue susan suzanne sydney tamara tammy tanya tara taylor ted teresa terri terry theresa thomas tiffany tim timothy tina todd tom tommy tony tracy travis trevor troy tyler valerie vanessa veronica vicki victor victoria vincent virginia vivian walter wanda warren wayne wendy wesley whitney will william willie wyatt yolanda zachary zoe abel agnes alberto alejandro alfred alma alvin amos andre antonio arlene armando arnold aubry barry beatrice bernice bert boyd brady brendan brenna brittni brittnei bryce byron calvin camille caryl cassandra cecilia celia chase cian clarence clifford clint clyde cole conrad corey cory cristina dallas damian damon dani darlene darryl dev dominic duane dwight edgar edna eduardo elias eliza ellie elliot elmer emilio emmett enrique ernest esther everett felix fernando flora floyd forrest francisco georgia gilbert gordon graham guadalupe guy hal harriet harvey hazel hector herbert herman homer hope hugh ida inez irene iris irma ivan jackson jade javier jayme jeanne jerome jesus jody joey johanna josephine juanita jude kai karl katelyn kellen kendra kent kerry kristyn kurt lacey lawrence leif leisl lena leroy lester levi lillian lloyd lonnie lorena lucia luther luz mabel mack mae malcolm manuel marc marcey marcia margarita marlene marshall mathew maxine melvin mercedes meredith mik milton mitchell nadia ned nelson nigel nina noel norman orlando otis pablo pearl peri perry preston priscilla quentin rafael ramon raul reed reggie renata rex roberto rochelle roland roman rosemary rudy sadie salvador sandy sangita sarinah saul sergio shay sheila sidney silvia skyler sonia spencer stacia stanley stella stuart sylvia terrence toby tracey trisha vera vernon vicky viola wade wallace wilma winston xavier yvonne zach".split(" ")
  );
  var NOT_A_NAME = /^(?:National|Park|Parks|Service|Services|Program|Office|Division|Department|Center|Centre|Region|Regional|Museum|Library|Archives?|University|College|School|Society|Association|Foundation|Conservancy|Trust|Friends|Fund|Media|Studios?|Productions?|Pictures|Films?|Video|Photo|Photography|Creative|Group|Company|Inc|LLC|Ltd|Corporation|Network|Television|TV|Radio|News|Press|Publishing|Partners|Institute|Project|Coalition|Alliance|Council|Commission|Committee|Board|Club|Band|Orchestra|Choir|Records|Music|Sound|Sounds|Audio|Design|Communications|Interpreting|Gallery|Church|Tribe|Nations?|County|City|State|States|Town|Village|Eastern|Western|Northern|Southern|Central|American|United|Historic|Historical|Heritage|Monument|Memorial|Trail|River|Lake|Island|Mountain|Valley|Canyon|Forest|Bay|Area|Conservation|Corps|Agency|Authority|Administration|Bureau|Government|Federal|Public|Digital|Visual|Entertainment|Labs?|Works|Workshop|Collective|Team|Staff|Crew|Volunteers|Podcast|Series|Smithsonian|History|Science|Nature|Art|Arts|Youth|Kids|Community|Stream|Mount|Mt|Fort|Saint|St|Point|Cape|Port|Mansion|House|Home|Farm|Gardens?|Hall|Castle|Hills?|Beach|Creek|Springs|Falls|Rock|Peak|Unit|Branch|Section)$/iu;
  var NAME_WORD = /^(?:\p{Lu}[\p{Ll}\p{M}'’‘ʻ]+(?:-?\p{Lu}[\p{Ll}\p{M}'’‘ʻ]+)*|\p{Lu}\.?|(?:\p{Lu}\.)+\p{Lu}[\p{Ll}\p{M}'’]+|de|da|di|du|del|der|van|von|la|le|Jr\.?|Sr\.?|II|III)$/u;
  var INITIAL = /^(?:\p{Lu}\.?|(?:\p{Lu}\.)+\p{Lu}[\p{Ll}\p{M}'’]+)$/u;
  var PARTS = /\s*(?:\/|\||;|,|&|\+|:|\s[-–—]\s|\(|\)|\[|\]|\bcourtesy of\b)\s*/iu;
  var JOINS = /(?:^|\s+)(?:and|with|for|by|from|via|at|on behalf of|funded by|in partnership with)(?:\s+|$)/iu;
  var MARK = "";
  var PARK_MARK = "";
  var PARK_NAME = new RegExp(
    `(?:\\p{Lu}[\\p{L}\\p{M}'’‘ʻ.-]*\\s+(?:(?:of|the|de|la|del)\\s+)?){1,5}${PARK_MARK}`,
    "gu"
  );
  var MARKS = new RegExp(`[${MARK}${PARK_MARK}]`, "gu");
  var nameWords = (text) => {
    const words = text.replace(/[.,]$/, "").split(/\s+/).filter((word) => word !== "");
    const first = words.findIndex((word) => !ROLES.has(word.toLowerCase().replace(/[.:]$/, "")));
    return first < 0 ? [] : words.slice(first);
  };
  var isPerson = (text, strict) => {
    const words = nameWords(text);
    if (words.length === 0 || words.length > 4) return false;
    const named = words.every(
      (word) => NAME_WORD.test(word) && !NOT_A_NAME.test(word) && !/['’]s$/u.test(word)
    );
    if (!named) return false;
    if (words.length === 1) return !strict;
    return words.some((word) => INITIAL.test(word)) || GIVEN_NAMES.has(words[0].toLowerCase().replace(/[.'’]/g, ""));
  };
  var isOffice = (text) => {
    const words = nameWords(text);
    return words.length > 0 && words.length <= 6 && words.every((word) => /^[\p{Lu}\d]/u.test(word));
  };
  var kindOfPiece = (piece, strict) => {
    const text = piece.trim();
    if (nameWords(text).length === 0) return "role";
    if (text.includes(MARK) || text.includes(PARK_MARK)) {
      const rest = text.replace(MARKS, " ").trim();
      return nameWords(rest).length === 0 || isPerson(rest, false) || isOffice(rest) ? "federal" : "other";
    }
    if (COMPANY.test(text)) return "other";
    if (/^\p{Lu}{4}$/u.test(text) && PARK_CODES.has(text.toLowerCase())) return "federal";
    return isPerson(text, strict) ? "person" : "other";
  };
  var MADE_FOR = new RegExp(`\\bfor\\s+(?:the\\s+)?[${MARK}${PARK_MARK}]`, "iu");
  var markFederal = (text) => text.normalize("NFC").replace(/\s+/g, " ").replace(NPS, MARK).replace(AGENCY, MARK).replace(PARK_UNIT, PARK_MARK).replace(PARK_NAME, PARK_MARK);
  var kindsOf = (text, strict) => {
    const marked = markFederal(text).trim().replace(/[.\s]+$/, "").replace(new RegExp(`([${MARK}${PARK_MARK}])\\s*[-‑–—]\\s*`, "gu"), "$1 / ");
    if (MADE_FOR.test(marked)) return ["other"];
    return marked.split(PARTS).filter((part) => part.trim() !== "").flatMap(
      (part) => NOT_FEDERAL.test(part) ? ["other"] : part.split(JOINS).map((piece) => kindOfPiece(piece, strict))
    );
  };
  var namesOnlyFederal = (text) => {
    const kinds = kindsOf(text, false);
    return kinds.includes("federal") && !kinds.includes("other");
  };
  var RIGHTS = /©|\(c\)\s*(?:19|20)\d\d|\bcopyright(?:ed|s)?\b|all rights reserved|creative\s*commons|\bcc[\s-]?by\b|licen[cs]ed? (?:under|by|from|to|for)\b|under (?:a |the )?(?:\S+ ){0,4}licen[cs]e\b|licen[cs]e code|used (?:with|by|under) permission|\bpermission (?:of|from)\b|with (?:the |their |kind )*permission|royalty[- ]free|stock (?:footage|music|video)/iu;
  var PUBLIC_DOMAIN_NOTE = /\bcopyright(?:\s+(?:info(?:rmation)?|status))?\s*:?\s*(?:none|public domain)\b|no known copyright restrictions|not (?:protected by|subject to) copyright|free of (?:known )?copyright(?: restrictions)?/giu;
  var NAME_START = `[\\p{Lu}\\d${MARK}${PARK_MARK}]`;
  var NAME_REST = `[\\p{L}\\p{M}\\d'’‘ʻ.${MARK}${PARK_MARK}-]*`;
  var NAMED = `((?:the\\s+)?${NAME_START}${NAME_REST}(?:(?:\\s*[,/&]\\s*|\\s+)(?:(?:and|the)\\s+)*${NAME_START}${NAME_REST})*)`;
  var SOURCE_WORDS = [
    "courtesy(?: of)?",
    "(?:footage|videos?|images?|photos?|photographs?|animation|music|songs?|soundtrack) by",
    "(?:music|songs?|soundtrack) from",
    "(?:produced|filmed|created|made|shot|recorded|directed|edited|narrated|presented|provided|funded|sponsored) by",
    "(?:music|songs?|soundtrack):",
    "(?:a )?(?:joint|collaboration (?:between|with)|partnership (?:between|with)|in (?:partnership|collaboration|cooperation|association) with)"
  ].map((words) => words.replace(/\b([a-z])/g, (letter) => `[${letter}${letter.toUpperCase()}]`)).join("|");
  var SOURCES = new RegExp(`(?:${SOURCE_WORDS})\\s+${NAMED}`, "gu");
  var describesNoOtherRights = (description) => {
    const text = markFederal(
      description.replace(/<[^>]*>/g, " ").replace(/&nbsp;/g, " ").replace(PUBLIC_DOMAIN_NOTE, " ")
    );
    if (RIGHTS.test(text)) return false;
    return [...text.matchAll(SOURCES)].every(
      (match) => !kindsOf(match[1] ?? "", true).includes("other")
    );
  };
  var credited = (credit) => {
    const text = (credit ?? "").trim();
    if (text === "") return true;
    if (RIGHTS.test(text.replace(PUBLIC_DOMAIN_NOTE, " "))) return false;
    return namesOnlyFederal(text);
  };

  // src/index.ts
  var API = "https://developer.nps.gov/api/v1";
  var AGENT = "Raygun/1.0 ( https://github.com/uvxszi/raygun )";
  var PAGE_SIZE = 50;
  var PARK = {
    kind: "select",
    key: "park",
    label: "Park",
    options: [
      { value: "all", label: "All parks" },
      ...PARKS.map(([code, name]) => ({ value: code, label: name }))
    ],
    default: "all"
  };
  var STATE = {
    kind: "select",
    key: "state",
    label: "State",
    options: [
      { value: "all", label: "All states and territories" },
      ...STATES.map(([code, name]) => ({ value: code, label: name }))
    ],
    default: "all"
  };
  var LANGUAGES = {
    english: "en",
    spanish: "es",
    french: "fr",
    german: "de",
    italian: "it",
    portuguese: "pt",
    russian: "ru",
    japanese: "ja",
    korean: "ko",
    mandarin: "zh",
    chinese: "zh",
    cantonese: "yue",
    vietnamese: "vi",
    tagalog: "tl",
    arabic: "ar",
    hawaiian: "haw",
    samoan: "sm",
    chamorro: "ch"
  };
  var apiKey = () => {
    const key = raygun.settings.get("apiKey")?.trim();
    if (!key) {
      return needsSetup(
        "The National Park Service's API needs your own free key: get one at nps.gov/subjects/developer/get-started.htm and enter it in this source's settings"
      );
    }
    return key;
  };
  var refused = () => needsSetup(
    "The National Park Service's API didn't accept the API key: check it in this source's settings"
  );
  var remember = (map, key, value, most) => {
    map.delete(key);
    map.set(key, value);
    if (map.size > most) map.delete(map.keys().next().value);
  };
  var pages = /* @__PURE__ */ new Map();
  var videos = /* @__PURE__ */ new Map();
  var ask = async (params) => {
    const key = apiKey();
    const response = await raygun.http.fetch(`${API}/multimedia/videos?${query(params)}`, {
      headers: { Accept: "application/json", "X-Api-Key": key, "User-Agent": AGENT }
    });
    const answer = await response.json().catch(() => null);
    const code = answer?.error?.code ?? "";
    if (response.status === 401 || response.status === 403 || code.startsWith("API_KEY_")) {
      refused();
    }
    if (response.status === 429 || code === "OVER_RATE_LIMIT") {
      throw new Error(
        "This API key has made the 1,000 requests the National Park Service's API allows in an hour; try again later"
      );
    }
    if (!response.ok) {
      throw new Error(`The National Park Service's API answered HTTP ${response.status}`);
    }
    if (!answer) throw new Error("The National Park Service's API answer couldn't be read");
    return answer;
  };
  var isFile = (url) => /^https?:\/\/\S+\.mp4$/i.test((url ?? "").trim());
  var filesOf = (video) => {
    const seen = /* @__PURE__ */ new Set();
    return (video.versions ?? []).filter((version) => {
      const url = version.url?.trim() ?? "";
      if (!isFile(url) || seen.has(url)) return false;
      seen.add(url);
      return true;
    });
  };
  var offered = (video) => Boolean(video.id) && filesOf(video).length > 0 && credited(video.credit) && describesNoOtherRights(video.description ?? "");
  var tidyTitle = (title) => title.replace(/^\s*\(video\)\s*/i, "").replace(/\s*\(\d{1,2}:\d{2}(?::\d{2})?\)\s*$/, "").replace(/\s+/g, " ").trim();
  var coverOf = (video) => {
    const splash = video.splashImage?.url?.trim() ?? "";
    if (/^https?:\/\//i.test(splash)) return splash;
    const legacy = filesOf(video).map((version) => version.url.trim()).find((url) => /\/legacy\/.+_\d+x\d+\.mp4$/i.test(url));
    return legacy ? legacy.replace(/_\d+x\d+\.mp4$/i, "_splash.jpg") : null;
  };
  var summaryOf = (video) => offered(video) ? [
    {
      id: video.id,
      title: tidyTitle(video.title ?? "") || video.id,
      coverUrl: coverOf(video)
    }
  ] : [];
  var keep = (video) => {
    if (video.id) remember(videos, video.id.toUpperCase(), video, 400);
  };
  var videoPage = async (params, page) => {
    const asked = Math.max(1, page);
    const all = { ...params, limit: PAGE_SIZE, start: (asked - 1) * PAGE_SIZE };
    const seen = query(all);
    const kept = pages.get(seen);
    if (kept) return kept;
    const answer = await ask(all);
    const found = answer.data ?? [];
    for (const video of found) if (offered(video)) keep(video);
    const total = Number(answer.total ?? 0);
    const listed = {
      items: found.flatMap(summaryOf),
      hasNextPage: found.length > 0 && asked * PAGE_SIZE < total
    };
    remember(pages, seen, listed, 60);
    return listed;
  };
  var videoOf = async (id) => {
    let video = videos.get(id.toUpperCase());
    if (!video) {
      const answer = await ask({ q: id, limit: 10 });
      video = (answer.data ?? []).find(
        (candidate) => candidate.id?.toUpperCase() === id.toUpperCase()
      );
      if (!video) throw new Error("This video is no longer in the National Park Service's library");
    }
    if (!offered(video)) {
      throw new Error(
        "This video names someone other than the National Park Service or a federal agency, so Raygun doesn't offer it"
      );
    }
    keep(video);
    return video;
  };
  var heightOf = (version) => {
    const url = version.url?.trim() ?? "";
    const sized = /_\d{2,4}x(\d{2,4})\.mp4$/i.exec(url);
    if (sized) return Number(sized[1]);
    const named = /(\d{3,4})p\.mp4$/i.exec(url);
    if (named) return Number(named[1]);
    const height = Number(version.heightPixels);
    return Number.isFinite(height) && height > 0 ? Math.round(height) : void 0;
  };
  var subtitlesOf = (video) => (video.captionFiles ?? []).flatMap((file) => {
    const url = file.url?.trim() ?? "";
    const kind = `${file.fileType ?? ""} ${url}`;
    if (!/^https?:\/\//i.test(url) || !/vtt|srt/i.test(kind)) return [];
    const language = (file.language ?? "").trim().toLowerCase();
    const label = language ? language.charAt(0).toUpperCase() + language.slice(1) : "Captions";
    const lang = LANGUAGES[language];
    return [lang ? { url, lang, label } : { url, label }];
  });
  var clock = (ms) => {
    const seconds = Math.round(ms / 1e3);
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor(seconds % 3600 / 60);
    const rest = String(seconds % 60).padStart(2, "0");
    return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${rest}` : `${minutes}:${rest}`;
  };
  var plainText = (html) => html.replace(/<br\s*\/?>/gi, "\n").replace(/<\/p>/gi, "\n\n").replace(/<[^>]*>/g, "").replace(/&nbsp;/g, " ").replace(/&quot;/g, '"').replace(/&#39;|&apos;|&rsquo;/g, "’").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").replace(/[^\S\n]+/g, " ").replace(/\n{3,}/g, "\n\n").trim();
  var GENERIC_TAGS = /* @__PURE__ */ new Set([
    "national",
    "park",
    "parks",
    "national park",
    "national parks",
    "national park service",
    "video",
    "videos",
    "b-roll",
    "broll"
  ]);
  var genresOf = (video) => {
    const seen = /* @__PURE__ */ new Set();
    return (video.tags ?? []).map((tag) => tag.replace(/\s+/g, " ").trim()).filter((tag) => {
      const key = tag.toLowerCase();
      if (key.length <= 3 || key.length > 40 || PARK_CODES.has(key) || GENERIC_TAGS.has(key)) {
        return false;
      }
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    }).slice(0, 8);
  };
  var descriptionOf = (video) => {
    const parks = (video.relatedParks ?? []).flatMap(
      (park) => park.fullName?.trim() ? [park.fullName.trim()] : []
    );
    const length = Number(video.durationMs);
    const facts = [
      parks.join(", "),
      Number.isFinite(length) && length > 0 ? clock(length) : ""
    ].filter(Boolean);
    const credit = video.credit?.trim() || "National Park Service";
    return [
      plainText(video.description ?? ""),
      facts.join(" · "),
      `Credit: ${credit}. Public domain in the United States: a work of the US government (17 U.S.C. § 105).`
    ].filter(Boolean).join("\n\n");
  };
  var VERSIONS = {
    described: {
      field: "audioDescriptionUrl",
      name: "With audio description",
      label: "Audio description"
    },
    asl: { field: "aslVideoUrl", name: "With ASL interpretation", label: "ASL" }
  };
  var videosOf = async (episodeId) => {
    const cut = episodeId.indexOf("/");
    const id = cut < 0 ? episodeId : episodeId.slice(0, cut);
    const kind = cut < 0 ? null : episodeId.slice(cut + 1);
    const video = await videoOf(id);
    if (kind === "described" || kind === "asl") {
      const version = VERSIONS[kind];
      const url = video[version.field]?.trim();
      if (!isFile(url)) {
        throw new Error("The National Park Service's library no longer has this version");
      }
      return [{ url, label: version.label }];
    }
    const subtitles = subtitlesOf(video);
    const files = filesOf(video).map((version) => ({ url: version.url.trim(), height: heightOf(version) })).sort((a, b) => (b.height ?? 0) - (a.height ?? 0));
    if (files.length === 0) {
      throw new Error("The National Park Service's library has no MP4 of this video");
    }
    return files.map(({ url, height }) => ({
      url,
      label: height ? `${height}p` : "MP4",
      quality: height,
      subtitles
    }));
  };
  registerProvider(
    defineProvider({
      kind: "video.source",
      id: "videos",
      name: "National Park Service",
      popular: ({ page }) => videoPage({}, page),
      search({ query: words, page, filters }) {
        const park = selectValue(filters, PARK);
        const state = selectValue(filters, STATE);
        const params = {};
        const text = words.trim();
        if (text) params.q = text;
        if (park !== "all") params.parkCode = park;
        if (state !== "all") params.stateCode = state;
        return videoPage(params, page);
      },
      filters: async () => [PARK, STATE],
      async details({ id }) {
        const video = await videoOf(id);
        return {
          id,
          title: tidyTitle(video.title ?? "") || id,
          coverUrl: coverOf(video),
          description: descriptionOf(video),
          genres: genresOf(video),
          type: "other",
          year: null,
          status: "completed",
          url: video.permalinkUrl?.trim() || `https://www.nps.gov/media/video/view.htm?id=${encodeURIComponent(id)}`
        };
      },
      async episodes({ id }) {
        const video = await videoOf(id);
        const episodes = [{ id, name: "Video" }];
        for (const [kind, version] of Object.entries(VERSIONS)) {
          if (isFile(video[version.field]))
            episodes.push({ id: `${id}/${kind}`, name: version.name });
        }
        return { episodes };
      },
      // One server, resolved at once: the Park Service serves its files itself.
      servers: async ({ episodeId }) => ({
        servers: [
          { id: episodeId, name: "National Park Service", videos: await videosOf(episodeId) }
        ]
      }),
      videos: async ({ serverId }) => ({ videos: await videosOf(serverId) })
    })
  );
})();
