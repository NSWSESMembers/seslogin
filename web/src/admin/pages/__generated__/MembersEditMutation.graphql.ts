/**
 * @generated SignedSource<<16a8ad8dcd5362eca793fbfe1e74d238>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type MembersEditMutation$variables = {
  firstName: string;
  id: string;
  lastName: string;
  memberNumber: string;
};
export type MembersEditMutation$data = {
  readonly updatePerson: {
    readonly firstName: string;
    readonly id: string;
    readonly lastName: string;
    readonly memberNumber: string | null | undefined;
  };
};
export type MembersEditMutation = {
  response: MembersEditMutation$data;
  variables: MembersEditMutation$variables;
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
  "name": "id"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "lastName"
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
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "lastName",
        "variableName": "lastName"
      },
      {
        "kind": "Variable",
        "name": "memberNumber",
        "variableName": "memberNumber"
      }
    ],
    "concreteType": "Person",
    "kind": "LinkedField",
    "name": "updatePerson",
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
    "name": "MembersEditMutation",
    "selections": (v4/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*:: as any*/),
      (v0/*:: as any*/),
      (v2/*:: as any*/),
      (v3/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "MembersEditMutation",
    "selections": (v4/*:: as any*/)
  },
  "params": {
    "cacheID": "1e698196b76aaf29626eae939aa92814",
    "id": null,
    "metadata": {},
    "name": "MembersEditMutation",
    "operationKind": "mutation",
    "text": "mutation MembersEditMutation(\n  $id: ID!\n  $firstName: String!\n  $lastName: String!\n  $memberNumber: String!\n) {\n  updatePerson(id: $id, firstName: $firstName, lastName: $lastName, memberNumber: $memberNumber) {\n    id\n    firstName\n    lastName\n    memberNumber\n  }\n}\n"
  }
};
})();

(node as any).hash = "723aa821795eef50b6bba3c2aa9a91ee";

export default node;
