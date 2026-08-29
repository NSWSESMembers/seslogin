/**
 * @generated SignedSource<<f910e1b6322702b53d7b512056a774d2>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityEditGuestMutation$variables = {
  comment?: string | null | undefined;
  endTime: number;
  guestName: string;
  id: string;
  startTime: number;
};
export type ActivityEditGuestMutation$data = {
  readonly updateGuestPeriod: {
    readonly comment: string | null | undefined;
    readonly endTime: number | null | undefined;
    readonly guestName: string | null | undefined;
    readonly id: string;
    readonly startTime: number;
  };
};
export type ActivityEditGuestMutation = {
  response: ActivityEditGuestMutation$data;
  variables: ActivityEditGuestMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "comment"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "endTime"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "guestName"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "startTime"
},
v5 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "comment",
        "variableName": "comment"
      },
      {
        "kind": "Variable",
        "name": "endTime",
        "variableName": "endTime"
      },
      {
        "kind": "Variable",
        "name": "guestName",
        "variableName": "guestName"
      },
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "startTime",
        "variableName": "startTime"
      }
    ],
    "concreteType": "Period",
    "kind": "LinkedField",
    "name": "updateGuestPeriod",
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
        "name": "endTime",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "comment",
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
    "argumentDefinitions": [
      (v0/*: any*/),
      (v1/*: any*/),
      (v2/*: any*/),
      (v3/*: any*/),
      (v4/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "ActivityEditGuestMutation",
    "selections": (v5/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v3/*: any*/),
      (v2/*: any*/),
      (v4/*: any*/),
      (v1/*: any*/),
      (v0/*: any*/)
    ],
    "kind": "Operation",
    "name": "ActivityEditGuestMutation",
    "selections": (v5/*: any*/)
  },
  "params": {
    "cacheID": "c4ed32b52518563235d041a082d9b6c1",
    "id": null,
    "metadata": {},
    "name": "ActivityEditGuestMutation",
    "operationKind": "mutation",
    "text": "mutation ActivityEditGuestMutation(\n  $id: ID!\n  $guestName: String!\n  $startTime: Int!\n  $endTime: Int!\n  $comment: String\n) {\n  updateGuestPeriod(id: $id, guestName: $guestName, startTime: $startTime, endTime: $endTime, comment: $comment) {\n    id\n    startTime\n    endTime\n    comment\n    guestName\n  }\n}\n"
  }
};
})();

(node as any).hash = "303836fa9f2a64c27f23c8a1dd1dfda7";

export default node;
