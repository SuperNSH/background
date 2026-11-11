import { effect, mount, signal, Text, View } from "nsh";
import { asBackgroundError, BackgroundFetch, FETCH_TASK_ID, memoryBackend } from "./src";

/** The demo's fetch runs on the memory backend; a dev machine has no OS scheduler to ask. */
const backend = memoryBackend();
const fetcher = new BackgroundFetch({ backend });

const message = signal("nothing yet");

function App() {
  effect(() => {
    void fetcher
      .configure({ minimumFetchIntervalMinutes: 15 }, (taskId) => message.set(taskId))
      // The memory backend fires nothing on its own; emit one fetch event as a timer would.
      .then(() => backend.emit({ taskId: FETCH_TASK_ID, timedOut: false, input: {} }));
  });
  const stop = () => {
    fetcher.stop().then(
      (stopped) => message.set(stopped ? "stopped the fetch" : "no fetch was running"),
      (error: unknown) => message.set(asBackgroundError(error).message),
    );
  };
  return (
    <View class="flex-1 gap-3 p-4">
      <Text testID="title" class="text-foreground">nsh-background</Text>
      <Text testID="message" class="text-foreground">{message()}</Text>
      <Text testID="stop" role="button" onPress={stop} class="rounded-(--radius) bg-muted px-3 py-2">
        Stop
      </Text>
    </View>
  );
}

mount(App);
