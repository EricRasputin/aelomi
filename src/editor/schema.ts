import {
  BlockNoteSchema,
  createBulletListItemBlockSpec,
  createCheckListItemBlockSpec,
  createCodeBlockSpec,
  createDividerBlockSpec,
  createHeadingBlockSpec,
  createImageBlockSpec,
  createInlineContentSpec,
  createNumberedListItemBlockSpec,
  createParagraphBlockSpec,
  createQuoteBlockSpec,
  defaultInlineContentSpecs,
  defaultStyleSpecs,
} from "@blocknote/core";
import { linkLabel, usePageTreeStore } from "../stores/page-tree";

export const pageLinkSpec = createInlineContentSpec(
  {
    type: "pageLink",
    propSchema: { targetPageId: { default: "" } },
    content: "none",
  },
  {
    render: (inline) => {
      const dom = document.createElement("span");
      dom.className = "page-link";
      const targetPageId = inline.props.targetPageId;
      const paint = () => {
        const label = linkLabel(
          usePageTreeStore.getState().source,
          targetPageId,
        );
        dom.textContent = label.kind === "loading" ? "" : label.text;
        dom.dataset.state = label.kind;
      };
      paint();
      const unsubscribe = usePageTreeStore.subscribe(paint);
      return { dom, destroy: unsubscribe };
    },
  },
);

export const editorSchema = BlockNoteSchema.create({
  blockSpecs: {
    paragraph: createParagraphBlockSpec(),
    heading: createHeadingBlockSpec({ levels: [1, 2, 3] }),
    bulletListItem: createBulletListItemBlockSpec(),
    numberedListItem: createNumberedListItemBlockSpec(),
    checkListItem: createCheckListItemBlockSpec(),
    quote: createQuoteBlockSpec(),
    codeBlock: createCodeBlockSpec(),
    divider: createDividerBlockSpec(),
    image: createImageBlockSpec(),
  },
  inlineContentSpecs: { ...defaultInlineContentSpecs, pageLink: pageLinkSpec },
  styleSpecs: defaultStyleSpecs,
});

export type EditorSchema = typeof editorSchema;
