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
import { paintLinkLabel, showLinkLabel } from "./link-labels";

export const pageLinkSpec = createInlineContentSpec(
  {
    type: "pageLink",
    propSchema: { targetPageId: { default: "" } },
    content: "none",
  },
  {
    render: (inline, _update, _editor, _node, getPos) => {
      const dom = document.createElement("span");
      dom.className = "page-link";
      const { targetPageId } = inline.props;
      // BlockNote's getPos returns undefined when it serializes outside the
      // editor for copy, drag or HTML export, which never calls destroy.
      if (getPos() === undefined) {
        paintLinkLabel(dom, targetPageId);
        return { dom };
      }
      return { dom, destroy: showLinkLabel(dom, targetPageId) };
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
