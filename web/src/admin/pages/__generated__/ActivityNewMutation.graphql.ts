/**
 * @generated SignedSource<<b41a1aa5b0ad8265df90c7cfe1adb9aa>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityNewMutation$variables = {
  categoryId: string;
  comment?: string | null | undefined;
  endTime: number;
  locationId: string;
  personId: string;
  startTime: number;
};
export type ActivityNewMutation$data = {
  readonly createPeriod: {
    readonly id: string;
  };
};
export type ActivityNewMutation = {
  response: ActivityNewMutation$data;
  variables: ActivityNewMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "categoryId"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "comment"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "endTime"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "locationId"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "personId"
},
v5 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "startTime"
},
v6 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "categoryId",
        "variableName": "categoryId"
      },
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
        "name": "locationId",
        "variableName": "locationId"
      },
      {
        "kind": "Variable",
        "name": "personId",
        "variableName": "personId"
      },
      {
        "kind": "Variable",
        "name": "startTime",
        "variableName": "startTime"
      }
    ],
    "concreteType": "Period",
    "kind": "LinkedField",
    "name": "createPeriod",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
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
      (v4/*: any*/),
      (v5/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "ActivityNewMutation",
    "selections": (v6/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v4/*: any*/),
      (v3/*: any*/),
      (v5/*: any*/),
      (v2/*: any*/),
      (v0/*: any*/),
      (v1/*: any*/)
    ],
    "kind": "Operation",
    "name": "ActivityNewMutation",
    "selections": (v6/*: any*/)
  },
  "params": {
    "cacheID": "be679e82f5e29e2b64338ce130f9adee",
    "id": null,
    "metadata": {},
    "name": "ActivityNewMutation",
    "operationKind": "mutation",
    "text": "mutation ActivityNewMutation(\n  $personId: ID!\n  $locationId: ID!\n  $startTime: Int!\n  $endTime: Int!\n  $categoryId: ID!\n  $comment: String\n) {\n  createPeriod(personId: $personId, locationId: $locationId, categoryId: $categoryId, startTime: $startTime, endTime: $endTime, comment: $comment) {\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "cbd668d7f00e576e64bc96850a4cee53";

export default node;
