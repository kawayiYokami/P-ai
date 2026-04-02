import { preloadExtendedLanguageIcons, setCustomComponents } from "markstream-vue";
import ChatShikiCodeBlockNode from "../components/ChatShikiCodeBlockNode.vue";

let registered = false;

export function registerChatMarkstreamComponents() {
  if (registered) return;

  setCustomComponents("chat-markstream", {
    code_block: ChatShikiCodeBlockNode,
  });
  void preloadExtendedLanguageIcons();
  registered = true;
}
