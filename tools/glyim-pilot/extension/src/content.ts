import { getAdapterForUrl } from './providers/adapter';
import { StreamWatcher } from './stream_watcher';

let activeWatcher: StreamWatcher | null = null;
