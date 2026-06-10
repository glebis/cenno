import React from 'react';
import {createRoot} from 'react-dom/client';
import {injectBasicCatalogStyles} from '@a2ui/web_core/v0_9/basic_catalog';
import App from './App';

// Install the stock catalog's token defaults at :where(:root).
// Without this, --a2ui-* vars are undefined and wrapper overrides win
// trivially; with it, the Q2 test proves overrides beat real defaults.
injectBasicCatalogStyles();

createRoot(document.getElementById('root')!).render(<App />);
