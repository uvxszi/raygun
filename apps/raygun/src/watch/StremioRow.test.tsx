import { fireEvent, render, screen } from '@testing-library/react';

import { StremioPoster } from './StremioRow';

describe('StremioPoster', () => {
  it('falls back to the placeholder when the image fails to load', () => {
    const { container } = render(<StremioPoster url="https://img.example/broken.jpg" />);
    const image = container.querySelector('img');
    expect(image).toHaveAttribute('src', 'https://img.example/broken.jpg');

    fireEvent.error(image!);
    expect(container.querySelector('img')).not.toBeInTheDocument();
    expect(screen.queryByRole('img')).not.toBeInTheDocument();
    expect(container.querySelector('svg')).toBeInTheDocument();
  });
});
