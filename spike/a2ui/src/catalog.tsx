/**
 * Spike question 1: can we register OUR OWN React component for a catalog type?
 *
 * Strategy: reuse the protocol-level ComponentApi (Zod schema) from
 * @a2ui/web_core's basic catalog, but provide our own React implementation
 * for `Button`, then compose a new Catalog under our own catalog id.
 */
import React from 'react';
import {Catalog} from '@a2ui/web_core/v0_9';
import {ButtonApi, BASIC_FUNCTIONS} from '@a2ui/web_core/v0_9/basic_catalog';
import {
  createComponentImplementation,
  Text,
  Card,
  Column,
  Row,
  TextField,
} from '@a2ui/react/v0_9';

/** Our own Button implementation, replacing the basic-catalog one. */
export const CennoButton = createComponentImplementation(
  ButtonApi,
  ({props, buildChild}) => {
    // props.action is already a ready-to-call () => void (Generic Binder).
    return (
      <button
        data-testid="cenno-button"
        className={`cenno-btn ${props.variant === 'primary' ? 'cenno-btn-primary' : ''}`}
        style={{
          background: 'var(--a2ui-color-primary, #333)',
          color: 'var(--a2ui-color-on-primary, #fff)',
          borderRadius: 'var(--a2ui-border-radius, 4px)',
          padding: 'var(--a2ui-spacing-m, 8px) var(--a2ui-spacing-l, 16px)',
          border: 'none',
        }}
        onClick={props.action}
        disabled={props.isValid === false}
      >
        {/* marker text proves OUR implementation rendered, not the stock one */}
        <span data-testid="cenno-button-marker">cenno::</span>
        {props.child ? buildChild(props.child) : null}
      </button>
    );
  },
);

/** Custom catalog: stock Text/Card/Column/Row/TextField + our Button. */
export const cennoCatalog = new Catalog(
  'cenno:catalog/v1',
  [Text, Card, Column, Row, TextField, CennoButton],
  BASIC_FUNCTIONS,
);
