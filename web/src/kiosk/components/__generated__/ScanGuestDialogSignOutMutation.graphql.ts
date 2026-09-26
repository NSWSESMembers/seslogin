/**
 * @generated SignedSource<<2e14a153dac77b672a6f9f7fedfd310e>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ScanGuestDialogSignOutMutation$variables = {
  id: string;
};
export type ScanGuestDialogSignOutMutation$data = {
  readonly scanGuestSignOut: {
    readonly endTime: number | null | undefined;
    readonly guestName: string | null | undefined;
    readonly id: string;
    readonly startTime: number;
    readonly version: number;
  };
};
export type ScanGuestDialogSignOutMutation = {
  response: ScanGuestDialogSignOutMutation$data;
  variables: ScanGuestDialogSignOutMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      }
    ],
    "concreteType": "Period",
    "kind": "LinkedField",
    "name": "scanGuestSignOut",
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
        "name": "endTime",
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
    "name": "ScanGuestDialogSignOutMutation",
    "selections": (v1/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "ScanGuestDialogSignOutMutation",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "664a8d0725312f0008cd3fc8e21cc406",
    "id": null,
    "metadata": {},
    "name": "ScanGuestDialogSignOutMutation",
    "operationKind": "mutation",
    "text": "mutation ScanGuestDialogSignOutMutation(\n  $id: ID!\n) {\n  scanGuestSignOut(id: $id) {\n    id\n    version\n    startTime\n    endTime\n    guestName\n  }\n}\n"
  }
};
})();

(node as any).hash = "8e65918a6bfbc991783ef268bc91c6c3";

export default node;
