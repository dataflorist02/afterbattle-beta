/** An Image that "loads" as soon as it gets its address (jsdom loads no images). */
export class LoadedImage {
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  private address = "";
  constructor(
    public width = 0,
    public height = 0,
  ) {}
  get src() {
    return this.address;
  }
  set src(value: string) {
    this.address = value;
    queueMicrotask(() => this.onload?.());
  }
}
