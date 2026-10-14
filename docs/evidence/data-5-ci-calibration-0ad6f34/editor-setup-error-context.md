# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: perf/keystroke.spec.ts >> EDIT-14 keystroke-to-paint on the editor-page-5000-blocks surface
- Location: tests/playwright/perf/keystroke.spec.ts:105:3

# Error details

```
Error: expect(locator).toBeVisible() failed

Locator: getByTestId('page-editor')
Expected: visible
Timeout: 5000ms
Error: element(s) not found

Call log:
  - Expect "toBeVisible" getByTestId('page-editor') with timeout 5000ms
  - waiting for getByTestId('page-editor')

```

```yaml
- banner: Aelomi
- status: IPC ok · core 0.1.0-mock · seq 1
- main:
  - article "Page":
    - heading "5,000 blocks" [level=1]
    - textbox "Page body":
      - checkbox [disabled]
      - paragraph: Lobo waje nogujo lohade. Keri hiseji jepale.
      - checkbox [disabled]
      - paragraph: Su fohaji putowe gutoti je newu tasi nufi.
      - paragraph:
        - strong: Bezu puva nogujo fepo le to kudi dure saze.
      - paragraph:
        - code: No karano bebuwe bojo je je je je to pejone some.
        - text: Sifumo honu bamo pajibi je buzaha homo du bodu. Vofeva kereri nedusa je ve pirufa jagefi nunu hiseji hejo je ni lifuru wurumi.

      # The intervening 5,000-block fixture projection is omitted from this excerpt.

      - paragraph: Zi je fenela bojo pedude dofe zite. Jujovo wurumi fewute wari waje sesu je jepale raza waje mora pedude to.
      - img "bbe74b2cde9c4a0ebd483ade9b3e5886.png"
      - text: •
      - paragraph: Keri wahi je jeragu je to pedude wopu witija tunupi.
      - text: •
      - paragraph: Bewe keri zuno hiseji bojo fekuzi gorire dado keri jagefi.
      - text: ◦
      - paragraph: Pedude je suduna rosa jepale hofapo zezu ko jozu to.
      - blockquote: Nogujo pirufa je nipohi lope to.
      - heading "Jagefi vowefi ni to." [level=3]
      - figure "Je dogo.":
        - img "01a8a1211652412b99f7a1f79146fe4f.png"
        - text: Je dogo.
      - img "d6458bff5e8b4a5abf6813ca1dd6fd5b.png"
      - code: let je = 78; let bekete = 534;
      - text: •
      - paragraph: Je ma to wo lelala gidope waje je na.
```
