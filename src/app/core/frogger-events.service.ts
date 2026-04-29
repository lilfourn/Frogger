import { Injectable } from "@angular/core";
import { listen, type Event, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  DirectoryListing,
  EventNames,
  IndexingState,
  OperationActivity,
} from "./frogger-api.types";

export interface FroggerEventPayloads {
  directoryListingProgress: DirectoryListing;
  indexingProgress: IndexingState;
  fileOperationProgress: OperationActivity;
  watcherUpdate: { paths: string[]; reason: string };
  settingsChanged: { keys: string[] };
  activityFailure: OperationActivity;
}

@Injectable({ providedIn: "root" })
export class FroggerEventsService {
  listenTo<TPayload>(eventName: string, handler: (payload: TPayload) => void): Promise<UnlistenFn> {
    return listen<TPayload>(eventName, (event: Event<TPayload>) => {
      handler(event.payload);
    });
  }

  listenToBootstrapEvents(
    events: EventNames,
    handlers: Partial<{
      [K in keyof FroggerEventPayloads]: (payload: FroggerEventPayloads[K]) => void;
    }>,
  ): Promise<UnlistenFn[]> {
    const registrations: Promise<UnlistenFn>[] = [];

    if (handlers.directoryListingProgress) {
      registrations.push(
        this.listenTo(events.directoryListingProgress, handlers.directoryListingProgress),
      );
    }

    if (handlers.indexingProgress) {
      registrations.push(this.listenTo(events.indexingProgress, handlers.indexingProgress));
    }

    if (handlers.fileOperationProgress) {
      registrations.push(
        this.listenTo(events.fileOperationProgress, handlers.fileOperationProgress),
      );
    }

    if (handlers.watcherUpdate) {
      registrations.push(this.listenTo(events.watcherUpdate, handlers.watcherUpdate));
    }

    if (handlers.settingsChanged) {
      registrations.push(this.listenTo(events.settingsChanged, handlers.settingsChanged));
    }

    if (handlers.activityFailure) {
      registrations.push(this.listenTo(events.activityFailure, handlers.activityFailure));
    }

    return Promise.all(registrations);
  }
}
