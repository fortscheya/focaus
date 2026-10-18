import type {ReactNode} from 'react';

export function StatusBadge({
  children,
  tone = 'neutral',
  dot = false,
}: {
  children: ReactNode;
  tone?: 'neutral' | 'success' | 'warning' | 'danger' | 'accent';
  dot?: boolean;
}) {
  return (
    <span className={`status-badge status-badge-${tone}`}>
      {dot ? <span aria-hidden="true" className="status-dot" /> : null}
      {children}
    </span>
  );
}
