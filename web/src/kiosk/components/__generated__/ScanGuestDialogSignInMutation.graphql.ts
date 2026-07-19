/**
 * @generated SignedSource<<53b253d8aa5676bf9e99caaed56af1a0>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ScanGuestDialogSignInMutation$variables = {
  name: string;
  reason?: string | null | undefined;
};
export type ScanGuestDialogSignInMutation$data = {
  readonly scanGuestSignIn: {
    readonly guestName: string | null | undefined;
    readonly id: string;
    readonly startTime: number;
  };
};
export type ScanGuestDialogSignInMutation = {
  response: ScanGuestDialogSignInMutation$data;
  variables: ScanGuestDialogSignInMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "name"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "reason"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "name",
        "variableName": "name"
      },
      {
        "kind": "Variable",
        "name": "reason",
        "variableName": "reason"
      }
    ],
    "concreteType": "Period",
    "kind": "LinkedField",
    "name": "scanGuestSignIn",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "startTime",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "guestName",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "ScanGuestDialogSignInMutation",
    "selections": (v1/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "ScanGuestDialogSignInMutation",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "ed7d6a1ce7214130f7fcd51863d535fe",
    "id": null,
    "metadata": {},
    "name": "ScanGuestDialogSignInMutation",
    "operationKind": "mutation",
    "text": "mutation ScanGuestDialogSignInMutation(\n  $name: String!\n  $reason: String\n) {\n  scanGuestSignIn(name: $name, reason: $reason) {\n    id\n    startTime\n    guestName\n  }\n}\n"
  }
};
})();

(node as any).hash = "a138669f131a0683878266ae065e15b6";

export default node;
