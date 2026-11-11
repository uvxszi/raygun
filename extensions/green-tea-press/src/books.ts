// Green Tea Press's free books with a web edition, and where each one states its licence (checked on
// those pages when this list was made). Think Python 2e's own pages name no licence; its page at
// Green Tea Press does. Books whose web edition doesn't say which licence it is under are left
// out.

/** How the web edition is made, which says where its contents and text are. */
export type Format = 'jupyter' | 'quarto' | 'hevea';

export type Book = {
  id: string;
  title: string;
  authors: string[];
  description: string;
  cover: string | null;
  /** The licence of the text, as the book states it. */
  licence: string;
  /** Where the book states it. */
  licenceAt: string;
  /** The web edition's folder; chapters are pages in it. */
  site: string;
  format: Format;
  subject: 'programming' | 'data' | 'science' | 'systems';
  year: number;
  /** Pages the contents list that aren't chapters (lists of notebooks). */
  skip?: string[];
};

const DOWNEY = 'Allen B. Downey';

export const BOOKS: Book[] = [
  {
    id: 'think-python-3e',
    title: 'Think Python (3rd edition)',
    authors: [DOWNEY],
    description:
      'An introduction to programming in Python for beginners, which teaches how to think like a computer scientist: values and expressions, functions, iteration, strings, lists, dictionaries, classes and debugging, with exercises in every chapter.',
    cover: 'https://greenteapress.com/wp/wp-content/uploads/2024/09/think_python_3e.jpg',
    licence: 'CC BY-NC-SA 4.0',
    licenceAt: 'https://allendowney.github.io/ThinkPython/',
    site: 'https://allendowney.github.io/ThinkPython/',
    format: 'jupyter',
    subject: 'programming',
    year: 2024,
    skip: ['blank.html', 'quizzes.html'],
  },
  {
    id: 'think-stats-3e',
    title: 'Think Stats (3rd edition)',
    authors: [DOWNEY],
    description:
      'Exploratory data analysis in Python: distributions, probability, relationships between variables, estimation, hypothesis testing, regression, time series and survival analysis, worked through with real data sets.',
    cover: 'https://greenteapress.com/wp/wp-content/uploads/2025/04/think_stats_3e_cover.jpg',
    licence: 'CC BY-NC-SA 4.0',
    licenceAt: 'https://allendowney.github.io/ThinkStats/chap01.html',
    site: 'https://allendowney.github.io/ThinkStats/',
    format: 'jupyter',
    subject: 'data',
    year: 2025,
  },
  {
    id: 'think-bayes-2e',
    title: 'Think Bayes (2nd edition)',
    authors: [DOWNEY],
    description:
      "Bayesian statistics in Python, taught with computation rather than calculus: Bayes's theorem, distributions, estimation, decision analysis, MCMC and approximate Bayesian computation, with many worked problems.",
    cover:
      'https://greenteapress.com/wp/wp-content/uploads/2021/04/think_bayes_2e_cover_medium.jpg',
    licence: 'CC BY-NC-SA 4.0',
    licenceAt: 'https://allendowney.github.io/ThinkBayes2/',
    site: 'https://allendowney.github.io/ThinkBayes2/',
    format: 'jupyter',
    subject: 'data',
    year: 2021,
  },
  {
    id: 'elements-of-data-science',
    title: 'Elements of Data Science',
    authors: [DOWNEY],
    description:
      'An introduction to data science in Python for people with no programming experience: variables, lists and arrays, dictionaries, plotting, DataFrames, distributions, regression, resampling and hypothesis testing.',
    cover: 'https://greenteapress.com/wp/wp-content/uploads/2026/01/eds_cover_small.png',
    licence: 'CC BY-NC-SA 4.0',
    licenceAt: 'https://allendowney.github.io/ElementsOfDataScience/',
    site: 'https://allendowney.github.io/ElementsOfDataScience/',
    format: 'jupyter',
    subject: 'data',
    year: 2024,
  },
  {
    id: 'modsimpy',
    title: 'Modeling and Simulation in Python',
    authors: [DOWNEY],
    description:
      'Modeling physical systems with simulation in Python: population growth, epidemics, thermal systems, pharmacokinetics, projectiles, drag and rotation, from building a model to validating it.',
    cover: 'https://greenteapress.com/wp/wp-content/uploads/2023/05/modsimpy_cover.png',
    licence: 'CC BY-NC-SA 4.0',
    licenceAt: 'https://allendowney.github.io/ModSimPy/',
    site: 'https://allendowney.github.io/ModSimPy/',
    format: 'jupyter',
    subject: 'science',
    year: 2023,
  },
  {
    id: 'think-java-2e',
    title: 'Think Java (2nd edition)',
    authors: [DOWNEY, 'Chris Mayfield'],
    description:
      'An introduction to computer science and programming in Java for beginners: variables, methods, conditionals, loops, arrays, recursion, objects and classes, with a strong focus on debugging.',
    cover:
      'https://greenteapress.com/wp/wp-content/uploads/2020/06/think_java_covr_large-229x300.png',
    licence: 'CC BY-NC-SA 4.0',
    licenceAt: 'https://greenteapress.com/wp/think-java-2e/',
    site: 'https://chrismayfield.github.io/ThinkJava2/',
    format: 'quarto',
    subject: 'programming',
    year: 2020,
  },
  {
    id: 'think-dsp',
    title: 'Think DSP',
    authors: [DOWNEY],
    description:
      'Digital signal processing in Python: sounds and signals, harmonics, noise, autocorrelation, the discrete cosine and Fourier transforms, filtering, convolution and modulation, explored with code and sound.',
    cover: 'https://greenteapress.com/wp/wp-content/uploads/2016/01/think_dsp_cover-229x300.jpg',
    licence: 'CC BY-NC-SA 4.0',
    licenceAt: 'https://greenteapress.com/thinkdsp/html/index.html',
    site: 'https://greenteapress.com/thinkdsp/html/',
    format: 'hevea',
    subject: 'science',
    year: 2016,
  },
  {
    id: 'think-complexity-2e',
    title: 'Think Complexity (2nd edition)',
    authors: [DOWNEY],
    description:
      'Complexity science in Python: graphs, small-world and scale-free networks, cellular automata, the Game of Life, self-organized criticality, agent-based models and the evolution of cooperation.',
    cover: 'https://greenteapress.com/wp/wp-content/uploads/2018/09/think_complexity_2e.jpg',
    licence: 'CC BY-NC-SA 4.0',
    licenceAt: 'https://greenteapress.com/complexity2/html/index.html',
    site: 'https://greenteapress.com/complexity2/html/',
    format: 'hevea',
    subject: 'science',
    year: 2018,
  },
  {
    id: 'think-os',
    title: 'Think OS',
    authors: [DOWNEY],
    description:
      'A brief introduction to operating systems for programmers: compilation, processes, virtual memory, file systems, memory management, caching, multitasking, threads and synchronization, with examples in C.',
    cover: null,
    licence: 'CC BY-NC-SA 4.0',
    licenceAt: 'https://greenteapress.com/thinkos/html/index.html',
    site: 'https://greenteapress.com/thinkos/html/',
    format: 'hevea',
    subject: 'systems',
    year: 2015,
  },
  {
    id: 'think-data-structures',
    title: 'Think Data Structures',
    authors: [DOWNEY],
    description:
      "Algorithms and information retrieval in Java: interfaces, analysis of algorithms, lists, trees, maps, hashing and persistence, built up into a web crawler and search engine over Wikipedia's pages.",
    cover:
      'https://greenteapress.com/wp/wp-content/uploads/2016/11/think_data_structures_cover-229x300.png',
    licence: 'CC BY-NC-SA 3.0',
    licenceAt: 'https://greenteapress.com/thinkdast/html/index.html',
    site: 'https://greenteapress.com/thinkdast/html/',
    format: 'hevea',
    subject: 'programming',
    year: 2017,
  },
  {
    id: 'dsirp',
    title: 'Data Structures and Information Retrieval in Python',
    authors: [DOWNEY],
    description:
      'Data structures and algorithms in Python, from analysis of algorithms and recursion to hash maps, heaps, Huffman codes, graphs and search, building up to a web crawler, indexer and PageRank.',
    cover: null,
    licence: 'CC BY-NC-SA 4.0',
    licenceAt: 'https://allendowney.github.io/DSIRP/',
    site: 'https://allendowney.github.io/DSIRP/',
    format: 'jupyter',
    subject: 'programming',
    year: 2021,
  },
  {
    id: 'astronomical-data',
    title: 'Astronomical Data in Python',
    authors: [DOWNEY],
    description:
      'Working with astronomical data in Python: querying the Gaia database, coordinates and units, proper motion, joining tables, photometry and visualization, replicating parts of a published analysis.',
    cover: null,
    licence: 'CC BY 4.0',
    licenceAt: 'https://allendowney.github.io/AstronomicalData/',
    site: 'https://allendowney.github.io/AstronomicalData/',
    format: 'jupyter',
    subject: 'science',
    year: 2021,
  },
  {
    id: 'think-python-2e',
    title: 'Think Python (2nd edition)',
    authors: [DOWNEY],
    description:
      'The earlier edition of Think Python, for Python 3: an introduction to programming and to thinking like a computer scientist, with case studies on interface design, word play and data structure selection.',
    cover: 'https://greenteapress.com/thinkpython2/think_python2_medium.jpg',
    licence: 'CC BY-NC 3.0',
    licenceAt: 'https://greenteapress.com/wp/think-python-2e/',
    site: 'https://greenteapress.com/thinkpython2/html/',
    format: 'hevea',
    subject: 'programming',
    year: 2015,
  },
  {
    id: 'think-stats-2e',
    title: 'Think Stats (2nd edition)',
    authors: [DOWNEY],
    description:
      'The earlier edition of Think Stats: exploratory data analysis in Python, from distributions and probability to estimation, hypothesis testing, regression, time series and survival analysis.',
    cover: 'https://greenteapress.com/thinkstats2/thinkstats2cover.jpg',
    licence: 'CC BY-NC-SA 4.0',
    licenceAt: 'https://greenteapress.com/thinkstats2/html/index.html',
    site: 'https://greenteapress.com/thinkstats2/html/',
    format: 'hevea',
    subject: 'data',
    year: 2014,
  },
  {
    id: 'think-bayes-1e',
    title: 'Think Bayes (1st edition)',
    authors: [DOWNEY],
    description:
      'The first edition of Think Bayes: Bayesian statistics made simple with Python, from computational statistics and estimation to prediction, hypothesis testing and hierarchical models.',
    cover: null,
    licence: 'CC BY-NC-SA 4.0',
    licenceAt: 'https://greenteapress.com/thinkbayes/html/index.html',
    site: 'https://greenteapress.com/thinkbayes/html/',
    format: 'hevea',
    subject: 'data',
    year: 2012,
  },
];
