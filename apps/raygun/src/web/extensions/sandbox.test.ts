import { SANDBOX_PAGE } from './sandbox';

describe('the sources’ sandbox page', () => {
  it('holds a script browsers can run', () => {
    const script = SANDBOX_PAGE.replace(/^[\s\S]*?<script>/, '').replace(/<\/script>$/, '');
    // A syntax error here would leave every source silently unloaded.
    expect(() => new Function(script)).not.toThrow();
  });
});
