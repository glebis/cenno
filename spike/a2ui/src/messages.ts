import {cennoCatalog} from './catalog';

/**
 * Hardcoded A2UI v0.9 flat component list: Card -> Column -> Text/TextField/Button.
 */
export const initialMessages = [
  {
    version: 'v0.9' as const,
    createSurface: {surfaceId: 'main', catalogId: cennoCatalog.id},
  },
  {
    version: 'v0.9' as const,
    updateComponents: {
      surfaceId: 'main',
      components: [
        {id: 'root', component: 'Card', child: 'col'},
        {id: 'col', component: 'Column', children: ['title', 'body', 'reply', 'submit']},
        {id: 'title', component: 'Text', variant: 'h2', text: {path: '/title'}},
        {id: 'body', component: 'Text', text: {path: '/body'}},
        {id: 'reply', component: 'TextField', label: 'Your reply', value: {path: '/draft'}},
        {
          id: 'submit',
          component: 'Button',
          variant: 'primary',
          child: 'submitLabel',
          action: {event: {name: 'submit', context: {draft: {path: '/draft'}}}},
        },
        {id: 'submitLabel', component: 'Text', text: 'Send'},
      ],
    },
  },
  {
    version: 'v0.9' as const,
    updateDataModel: {
      surfaceId: 'main',
      path: '/',
      value: {
        title: 'Hello from cenno spike',
        body: 'Initial body text (before any patch).',
        draft: '',
      },
    },
  },
];

/**
 * Spike question 3: incremental patch of ONE component by id.
 * NOTE: the processor REPLACES the matched component's properties wholesale
 * (`existing.properties = properties`), so the patch must carry the full
 * property set for that component — but only that component.
 */
export const patchBodyMessage = (stamp: string) => ({
  version: 'v0.9' as const,
  updateComponents: {
    surfaceId: 'main',
    components: [{id: 'body', component: 'Text', text: `PATCHED ${stamp}`}],
  },
});

/** Spike question 4a: a v0.8-protocol-shaped message (beginRendering era). */
export const v08StyleMessage = {
  version: 'v0.8',
  beginRendering: {surfaceId: 'main', root: 'root'},
} as any;

/** Spike question 4b: v0.9-shaped payload but with a wrong/missing version tag. */
export const wrongVersionPatch = {
  version: 'v0.42',
  updateComponents: {
    surfaceId: 'main',
    components: [{id: 'body', component: 'Text', text: 'PATCHED-BY-WRONG-VERSION'}],
  },
} as any;
