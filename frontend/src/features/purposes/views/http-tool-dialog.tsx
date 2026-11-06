import { useState } from 'react';

import { Button } from '../../../shared/ui/button';
import { CheckboxField } from '../../../shared/ui/checkbox-field';
import { FormDialog } from '../../../shared/ui/form-dialog';
import { SelectField } from '../../../shared/ui/select-field';
import { TextField } from '../../../shared/ui/text-field';
import { METHODS, choose } from '../state/choices';
import { LIMITS, type HttpToolDraft } from '../state/models';
import {
  checkHttpTool,
  hostAllowed,
  hostOf,
  newHttpTool,
  newToolInput,
} from '../state/purpose-edits';
import { ToolInputFields } from './tool-input-fields';

interface HttpToolDialogProps {
  /** The tool to edit; without it the dialog adds one. */
  tool?: HttpToolDraft | undefined;
  /** Names already taken by the purpose's other tools and the built-ins. */
  taken: readonly string[];
  builtins: readonly string[];
  /** The hosts tools may call, to warn before the server does. */
  allowedHosts: readonly string[];
  onSave: (tool: HttpToolDraft) => void;
}

/** What the URL field says about its host, before anything is saved. */
function hostNote(url: string, allowed: readonly string[]): string | undefined {
  const host = hostOf(url);
  if (host === '' || hostAllowed(host, allowed)) return undefined;
  return `${host} isn't on the tool host allow-list, so a version can't be saved with it.`;
}

type Errors = ReturnType<typeof checkHttpTool>;

/**
 * Adds or edits a custom HTTP tool: what the LLM calls it and reads about it, where it calls,
 * the secret it sends (by name only), and its inputs. Nothing is called until M4b.
 */
export function HttpToolDialog({
  tool,
  taken,
  builtins,
  allowedHosts,
  onSave,
}: HttpToolDialogProps) {
  const [draft, setDraft] = useState<HttpToolDraft>(() => newHttpTool(LIMITS.timeoutMs.default));
  const [errors, setErrors] = useState<Errors>({});
  const set = <K extends keyof HttpToolDraft>(field: K, value: HttpToolDraft[K]) => {
    setDraft((current) => ({ ...current, [field]: value }));
  };
  return (
    <FormDialog
      triggerLabel={tool === undefined ? 'Add HTTP tool' : 'Edit'}
      triggerAriaLabel={tool === undefined ? undefined : `Edit ${tool.name}`}
      title={tool === undefined ? 'Add HTTP tool' : `Edit ${tool.name}`}
      submitLabel={tool === undefined ? 'Add tool' : 'Save tool'}
      onOpenChange={(open) => {
        if (!open) return;
        setDraft(tool ?? newHttpTool(LIMITS.timeoutMs.default));
        setErrors({});
      }}
      onSubmit={() => {
        const found = checkHttpTool(draft, taken, builtins);
        setErrors(found);
        if (Object.keys(found).length > 0) return false;
        onSave({ ...draft, name: draft.name.trim(), url: draft.url.trim() });
        return true;
      }}
    >
      <fieldset className="flex flex-col gap-4">
        <legend className="sr-only">Tool</legend>
        <TextField
          label="Tool name"
          value={draft.name}
          onValueChange={(name) => {
            set('name', name);
          }}
          description="What the LLM calls it, e.g. book_slot."
          error={errors.name}
          autoComplete="off"
        />
        <TextField
          label="What it does"
          value={draft.description}
          onValueChange={(description) => {
            set('description', description);
          }}
          maxLength={LIMITS.toolDescription}
          error={errors.description}
          autoComplete="off"
        />
        <div className="grid grid-cols-3 gap-3">
          <SelectField
            label="Method"
            value={draft.method}
            onValueChange={(value) => {
              const method = choose(METHODS, value);
              if (method !== undefined) set('method', method);
            }}
            options={METHODS}
          />
          <div className="col-span-2">
            <TextField
              label="URL"
              value={draft.url}
              onValueChange={(url) => {
                set('url', url);
              }}
              maxLength={LIMITS.url}
              inputMode="url"
              description={hostNote(draft.url, allowedHosts)}
              error={errors.url}
              autoComplete="off"
            />
          </div>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <TextField
            label="Timeout (ms)"
            type="number"
            min={LIMITS.timeoutMs.min}
            max={LIMITS.timeoutMs.max}
            value={String(draft.timeoutMs)}
            onValueChange={(value) => {
              set('timeoutMs', Number(value));
            }}
          />
          <TextField
            label="Secret name"
            value={draft.secret}
            onValueChange={(secret) => {
              set('secret', secret);
            }}
            description="The name in your secret store, never its value."
            autoComplete="off"
          />
        </div>
        <CheckboxField
          label="Confirm with the caller first"
          checked={draft.confirm}
          onCheckedChange={(confirm) => {
            set('confirm', confirm);
          }}
        />
      </fieldset>
      <ToolInputFields
        inputs={draft.inputs}
        onChange={(inputs) => {
          set('inputs', inputs);
        }}
        error={errors.inputs}
      />
      {draft.inputs.length < LIMITS.inputs && (
        <Button
          onClick={() => {
            set('inputs', [...draft.inputs, newToolInput()]);
          }}
        >
          Add input
        </Button>
      )}
    </FormDialog>
  );
}
