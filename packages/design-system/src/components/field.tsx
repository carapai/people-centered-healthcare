import { Field as ArkField } from '@ark-ui/react/field'

import { field } from '../../styled-system/recipes'
import { slot } from './slot'

const classes = field()

export const Field = {
  Root: slot(ArkField.Root, classes.root),
  Label: slot(ArkField.Label, classes.label),
  RequiredIndicator: slot(ArkField.RequiredIndicator, classes.requiredIndicator),
  Input: slot(ArkField.Input, classes.input),
  Textarea: slot(ArkField.Textarea, classes.textarea),
  Select: slot(ArkField.Select, classes.select),
  HelperText: slot(ArkField.HelperText, classes.helperText),
  ErrorText: slot(ArkField.ErrorText, classes.errorText),
}
