/**
 * @generated SignedSource<<8d504abb2c4569b93303293d90ac0f88>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type HelpFormMutation$variables = {
  message: string;
  subject: string;
};
export type HelpFormMutation$data = {
  readonly submitFeedback: {
    readonly number: number;
    readonly reference: string;
  };
};
export type HelpFormMutation = {
  response: HelpFormMutation$data;
  variables: HelpFormMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "message"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "subject"
},
v2 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "message",
        "variableName": "message"
      },
      {
        "kind": "Variable",
        "name": "subject",
        "variableName": "subject"
      }
    ],
    "concreteType": "SubmittedFeedback",
    "kind": "LinkedField",
    "name": "submitFeedback",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "number",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "reference",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*: any*/),
      (v1/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "HelpFormMutation",
    "selections": (v2/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*: any*/),
      (v0/*: any*/)
    ],
    "kind": "Operation",
    "name": "HelpFormMutation",
    "selections": (v2/*: any*/)
  },
  "params": {
    "cacheID": "47791c10933d4b6874ae5d8acf098161",
    "id": null,
    "metadata": {},
    "name": "HelpFormMutation",
    "operationKind": "mutation",
    "text": "mutation HelpFormMutation(\n  $subject: String!\n  $message: String!\n) {\n  submitFeedback(subject: $subject, message: $message) {\n    number\n    reference\n  }\n}\n"
  }
};
})();

(node as any).hash = "a97e23018eb964a24eccc670bbffe421";

export default node;
