/**
 * @generated SignedSource<<c2c9b10b68b480e0a1f95ef9b749535a>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type MembersNewMutation$variables = {
  firstName: string;
  lastName: string;
  locationId: string;
  memberNumber: string;
};
export type MembersNewMutation$data = {
  readonly createPerson: {
    readonly firstName: string;
    readonly id: string;
    readonly lastName: string;
    readonly memberNumber: string | null | undefined;
  };
};
export type MembersNewMutation = {
  response: MembersNewMutation$data;
  variables: MembersNewMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "firstName"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "lastName"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "locationId"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "memberNumber"
},
v4 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "firstName",
        "variableName": "firstName"
      },
      {
        "kind": "Variable",
        "name": "lastName",
        "variableName": "lastName"
      },
      {
        "kind": "Variable",
        "name": "locationId",
        "variableName": "locationId"
      },
      {
        "kind": "Variable",
        "name": "memberNumber",
        "variableName": "memberNumber"
      }
    ],
    "concreteType": "Person",
    "kind": "LinkedField",
    "name": "createPerson",
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
        "name": "firstName",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "lastName",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "memberNumber",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v3/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "MembersNewMutation",
    "selections": (v4/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
      (v3/*:: as any*/),
      (v2/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "MembersNewMutation",
    "selections": (v4/*:: as any*/)
  },
  "params": {
    "cacheID": "ee2ca99466d5e4eb6da07f583fc3d009",
    "id": null,
    "metadata": {},
    "name": "MembersNewMutation",
    "operationKind": "mutation",
    "text": "mutation MembersNewMutation(\n  $firstName: String!\n  $lastName: String!\n  $memberNumber: String!\n  $locationId: ID!\n) {\n  createPerson(firstName: $firstName, lastName: $lastName, memberNumber: $memberNumber, locationId: $locationId) {\n    id\n    firstName\n    lastName\n    memberNumber\n  }\n}\n"
  }
};
})();

(node as any).hash = "09ff009b2ff4ef9f7e6deed58deddb62";

export default node;
