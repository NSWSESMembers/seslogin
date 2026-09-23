/**
 * @generated SignedSource<<0c10ff4cf1f91c4b87cbe5be8d05dfa0>>
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
    readonly version: number;
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
        "name": "version",
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
    "cacheID": "e775a544c52fc246d8ff637e9c1332fc",
    "id": null,
    "metadata": {},
    "name": "ScanGuestDialogSignInMutation",
    "operationKind": "mutation",
    "text": "mutation ScanGuestDialogSignInMutation(\n  $name: String!\n  $reason: String\n) {\n  scanGuestSignIn(name: $name, reason: $reason) {\n    id\n    version\n    startTime\n    guestName\n  }\n}\n"
  }
};
})();

(node as any).hash = "ff6e025c181a278ad62499863099eac8";

export default node;
