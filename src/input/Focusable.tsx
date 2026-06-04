import { onMount, type JSX } from "solid-js";
import { focusedId, registerFocusable, type FocusableNode } from "./focus";

interface FocusableProps extends Omit<FocusableNode, "element"> {
  children: JSX.Element;
  class?: string;
  focusedClass?: string;
}

export function Focusable(props: FocusableProps) {
  let ref!: HTMLDivElement;

  onMount(() => {
    registerFocusable({
      id: props.id,
      element: ref,
      onActivate: props.onActivate,
    });
  });

  const isFocused = () => focusedId() === props.id;

  return (
    <div
      ref={ref}
      class={`${props.class ?? ""} ${isFocused() ? props.focusedClass ?? "ring-2 ring-ring" : ""}`.trim()}
    >
      {props.children}
    </div>
  );
}
