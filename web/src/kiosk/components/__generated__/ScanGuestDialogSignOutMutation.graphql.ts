/**
 * @generated SignedSource<<dfd9250279951a9871e7562bac253e86>>
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
    readonly id: string;
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
        "name": "endTime",
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
    "cacheID": "7a5d71eae72a90a4a3e09828033c4402",
    "id": null,
    "metadata": {},
    "name": "ScanGuestDialogSignOutMutation",
    "operationKind": "mutation",
    "text": "mutation ScanGuestDialogSignOutMutation(\n  $id: ID!\n) {\n  scanGuestSignOut(id: $id) {\n    id\n    endTime\n  }\n}\n"
  }
};
})();

(node as any).hash = "d5a42a48fbdf087d63c20764740e6b1b";

export default node;
