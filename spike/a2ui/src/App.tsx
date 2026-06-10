import React, {useEffect, useState} from 'react';
import {MessageProcessor} from '@a2ui/web_core/v0_9';
import {A2uiSurface} from '@a2ui/react/v0_9';
import {cennoCatalog} from './catalog';
import {
  initialMessages,
  patchBodyMessage,
  v08StyleMessage,
  wrongVersionPatch,
} from './messages';

declare global {
  interface Window {
    __spike: {
      processor: MessageProcessor<any>;
      actions: unknown[];
      patch: (stamp: string) => void;
      feedV08: () => string;
      feedWrongVersion: () => string;
    };
  }
}

export default function App() {
  const [actions, setActions] = useState<unknown[]>([]);

  const [processor] = useState(() => {
    const p = new MessageProcessor([cennoCatalog], action => {
      setActions(prev => [...prev, action]);
    });
    p.processMessages(initialMessages as any);
    return p;
  });

  const [surfaces, setSurfaces] = useState(() =>
    Array.from(processor.model.surfacesMap.values()),
  );

  useEffect(() => {
    const sync = () =>
      setSurfaces(Array.from(processor.model.surfacesMap.values()));
    const created = processor.onSurfaceCreated(sync);
    const deleted = processor.onSurfaceDeleted(sync);
    return () => {
      created.unsubscribe();
      deleted.unsubscribe();
    };
  }, [processor]);

  useEffect(() => {
    // Expose hooks for headless verification.
    window.__spike = {
      processor,
      actions,
      patch: (stamp: string) =>
        processor.processMessages([patchBodyMessage(stamp)] as any),
      feedV08: () => {
        try {
          processor.processMessages([v08StyleMessage]);
          return 'no-throw';
        } catch (e) {
          return `threw: ${e}`;
        }
      },
      feedWrongVersion: () => {
        try {
          processor.processMessages([wrongVersionPatch]);
          return 'no-throw';
        } catch (e) {
          return `threw: ${e}`;
        }
      },
    };
  }, [processor, actions]);

  return (
    <div>
      <h1>A2UI spike harness</h1>

      {/* Spike question 2: CSS custom properties on a WRAPPER element.
          Defaults ship at :where(:root) so wrapper values must win for
          all descendants via normal CSS inheritance. */}
      <div
        id="themed-wrapper"
        style={
          {
            '--a2ui-color-primary': 'rgb(255, 0, 128)',
            '--a2ui-color-on-primary': 'rgb(0, 255, 0)',
            '--a2ui-border-radius': '13px',
            '--a2ui-color-surface': 'rgb(10, 20, 30)',
            '--a2ui-color-on-surface': 'rgb(200, 210, 220)',
          } as React.CSSProperties
        }
      >
        {surfaces.length === 0 && <div>Waiting for agent...</div>}
        {surfaces.map(surface => (
          <A2uiSurface key={surface.id} surface={surface} />
        ))}
      </div>

      <pre id="action-log">{JSON.stringify(actions, null, 2)}</pre>
    </div>
  );
}
