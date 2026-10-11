import blocksJson from "../../fixtures/generated/page-5000-blocks.json?raw";
import { fixtureParagraphs } from "./fixture-text";
import { KeystrokeSurface } from "./keystroke-surface";

const paragraphs = fixtureParagraphs(JSON.parse(blocksJson));

export default function KeystrokePage5000() {
  return <KeystrokeSurface paragraphs={paragraphs} />;
}
