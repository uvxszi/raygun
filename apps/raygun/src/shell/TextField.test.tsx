import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import type { FC } from 'react';

import '../i18n';
import { TextField } from './TextField';
import type { TextFieldProps } from './TextField';

/** A field that keeps what's typed, as every caller does. */
const Field: FC<Partial<TextFieldProps> & { initial?: string }> = ({ initial = '', ...props }) => {
  const [value, setValue] = useState(initial);
  return <TextField aria-label="Address" {...props} value={value} onChange={setValue} />;
};

describe('TextField', () => {
  it('shows a clear button only while there is text, and clearing keeps the keyboard up', async () => {
    const user = userEvent.setup();
    const onClear = vi.fn();
    render(<Field onClear={onClear} />);
    const field = screen.getByRole('textbox', { name: 'Address' });
    expect(screen.queryByRole('button', { name: 'Clear Address' })).not.toBeInTheDocument();

    await user.type(field, 'https://example.org/a/very/long/address/index.json');
    await user.click(screen.getByRole('button', { name: 'Clear Address' }));
    expect(field).toHaveValue('');
    expect(field).toHaveFocus();
    expect(onClear).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('button', { name: 'Clear Address' })).not.toBeInTheDocument();
  });

  it('has nothing to clear while disabled', () => {
    render(<Field initial="kept" disabled />);
    expect(screen.queryByRole('button', { name: 'Clear Address' })).not.toBeInTheDocument();
  });

  it('clears a search field on Escape, and hands the next Escape on', async () => {
    const user = userEvent.setup();
    const around = vi.fn();
    render(
      <div onKeyDown={(event) => event.key === 'Escape' && around()}>
        <Field type="search" initial="daft punk" />
      </div>,
    );
    const field = screen.getByRole('searchbox', { name: 'Address' });
    field.focus();
    await user.keyboard('{Escape}');
    expect(field).toHaveValue('');
    expect(field).toHaveFocus();
    expect(around).not.toHaveBeenCalled();
    // Nothing left to clear: Escape is the page's (a sheet closes, search mode ends).
    await user.keyboard('{Escape}');
    expect(around).toHaveBeenCalledTimes(1);
  });

  it("leaves Escape alone in other fields, and passes the caller's keys on", async () => {
    const user = userEvent.setup();
    const onKeyDown = vi.fn();
    render(<Field initial="name" onKeyDown={onKeyDown} />);
    const field = screen.getByRole('textbox', { name: 'Address' });
    field.focus();
    await user.keyboard('{Escape}');
    expect(field).toHaveValue('name');
    expect(onKeyDown).toHaveBeenCalled();
  });
});
